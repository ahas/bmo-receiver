//! The one way a write route reaches a room (docs/receiver.md, 「Console rooms and accounts」; docs/api.md, 「Rooms」):
//! every route that changes a room, a message in it or what is kept of it is handed the room through the guard here,
//! which reads who the write acts for off the request and refuses another account's private room before the handler
//! holds anything. Who acts is the account the call names, as `account` or `by` in its query or body or as the relay's
//! `Owner-Key`, all of them one account where several are named; the owner on his own road; or no one, a unit acting
//! as itself under the receiver's token.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use axum::{
  body::{Body, Bytes},
  extract::{FromRequest, FromRequestParts, Multipart, Path, Request, rejection::BytesRejection},
  http::{Extensions, HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE},
};
use receiver_codes::RoomAccountRefusal;
use serde_json::Value;

use super::{Refusal, Served, code, refused, store};
use crate::accounts::{self, Privacy};
use crate::receiver::{NotReceived, Receiver};

/// Who a write acts for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Actor {
  /// An account the console acts for.
  Account(i64),
  /// No account named.
  Unnamed,
}

/// Who a write acts for and what it carries: the one way a write route reads its body, so no route that changes a
/// room reads one without the guard. A write on the owner's own road, `POST /owner/messages` and its kin, writes in
/// his private room always, and elsewhere for the account it names, or for his account where it names none. A body
/// of words is read whole; a multipart form is left unread, for its route to read as it arrives (docs/api.md,
/// 「Sends」).
pub(super) struct Writer {
  actor: Actor,
  owners_road: bool,
  body: Bytes,
  /// The form left unread, held so the writer is shared across the handler's waits.
  form: Mutex<Option<Body>>,
  kind: Option<HeaderValue>,
  /// The request's extensions, the ceiling the route reads its body under among them.
  extensions: Extensions,
}

/// A room a write may change, refused before the handler holds it where it is another account's private room: the
/// one way a write route on `/rooms/{roomId}` reaches its room.
pub(super) struct WritableRoom {
  pub room: i64,
  pub writer: Writer,
}

fn bad(why: &str) -> Refusal {
  refused(StatusCode::BAD_REQUEST, "bad-request", why)
}

// A body the framework could not read: past the ceiling the route reads its body under is 413 `too-large`
// (docs/api.md, 「Sends」), anything else not the shape the route takes.
fn unread(why: &BytesRejection) -> Refusal {
  match why.status() {
    StatusCode::PAYLOAD_TOO_LARGE => refused(StatusCode::PAYLOAD_TOO_LARGE, "too-large", &why.body_text()),
    _ => bad(&why.body_text()),
  }
}

fn reached(why: NotReceived) -> Refusal {
  match why {
    NotReceived::Store(why) => store(why),
    why => refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string()),
  }
}

// An account as a request names it: a number, or a number written as text; anything else names none.
fn account_of(named: Option<&Value>) -> Option<i64> {
  match named? {
    Value::Number(number) => number.as_i64(),
    Value::String(text) => text.trim().parse::<i64>().ok(),
    _ => None,
  }
}

// Whether a request carries a multipart form, which its route reads as it arrives.
fn is_form(headers: &HeaderMap) -> bool {
  headers
    .get(CONTENT_TYPE)
    .and_then(|kind| kind.to_str().ok())
    .is_some_and(|kind| kind.trim_start().to_ascii_lowercase().starts_with("multipart/form-data"))
}

// The account the request names in its query, its body and the relay's `Owner-Key`: refused where they name
// different ones.
fn named_in(headers: &HeaderMap, query: Option<&str>, body: &Bytes) -> Result<Option<i64>, Refusal> {
  let queried: HashMap<String, String> =
    serde_urlencoded::from_str(query.unwrap_or_default()).unwrap_or_default();
  let bodied: Value = if body.is_empty() {
    Value::Null
  } else {
    serde_json::from_slice(body).unwrap_or(Value::Null)
  };
  let keyed = headers
    .get("owner-key")
    .and_then(|value| value.to_str().ok())
    .and_then(|key| key.trim().parse::<i64>().ok());
  let named = ["account", "by"]
    .iter()
    .flat_map(|field| {
      [
        account_of(queried.get(*field).map(|text| Value::String(text.clone())).as_ref()),
        account_of(bodied.get(*field)),
      ]
    })
    .chain([keyed])
    .flatten()
    .collect::<Vec<i64>>();

  match named.split_first() {
    None => Ok(None),
    Some((first, rest)) if rest.iter().all(|other| other == first) => Ok(Some(*first)),
    Some(_) => Err(bad("a write names one account, in its query, its body or Owner-Key alike")),
  }
}

impl Writer {
  /// The account the write acts for, where it names one.
  pub fn account(&self) -> Option<i64> {
    match self.actor {
      Actor::Account(account) => Some(account),
      Actor::Unnamed => None,
    }
  }

  /// The body of words the write carries; empty for a form.
  pub fn body(&self) -> &Bytes {
    &self.body
  }

  /// The write on the owner's own road.
  pub fn owners(mut self) -> Self {
    self.owners_road = true;

    self
  }

  /// The write for the account a form names where the request named none, refused where it named another.
  pub fn for_account(mut self, account: Option<i64>) -> Result<Self, Refusal> {
    match (self.actor, account) {
      (_, None) => Ok(self),
      (Actor::Account(named), Some(account)) if named != account => {
        Err(bad("a write names one account, in its query, its body or Owner-Key alike"))
      }
      (_, Some(account)) => {
        self.actor = Actor::Account(account);
        Ok(self)
      }
    }
  }

  /// A room the write changes, refused 403 `private-room` where it is another account's private room, or a sealed
  /// one, for any account. The owner's own road writes in his private room always, and in a room private to an
  /// account only where the web unit links that account to his sender, asked only then; a unit acting as itself is
  /// refused nowhere.
  pub async fn room(&self, receiver: &Receiver, room: i64) -> Result<(), Refusal> {
    if self.owners_road && accounts::owners_room(receiver, room).await {
      return Ok(());
    }

    if !self.owners_road && self.actor == Actor::Unnamed {
      return Ok(());
    }

    let privacy = accounts::privacy(receiver, room).await.map_err(reached)?;
    let account = match (self.actor, privacy) {
      (_, Privacy::Shared) => return Ok(()),
      (Actor::Account(account), _) => Some(account),
      (Actor::Unnamed, Privacy::Of(_)) => accounts::owner_account(receiver).await.map_err(reached)?,
      (Actor::Unnamed, Privacy::Sealed) => None,
    };

    if matches!(privacy, Privacy::Of(own) if account == Some(own)) {
      return Ok(());
    }

    Err(code(&RoomAccountRefusal::PrivateRoom {
      room,
      account: account.unwrap_or_default(),
    }))
  }

  /// The multipart form the write carries, to be read as it arrives under the ceiling its route sets; a body of
  /// words is no form, and a form is read once.
  pub async fn form(&mut self) -> Result<Multipart, Refusal> {
    let body = self
      .form
      .get_mut()
      .unwrap_or_else(PoisonError::into_inner)
      .take()
      .ok_or_else(|| bad("the request carries no multipart form"))?;
    let mut request = Request::new(body);

    *request.extensions_mut() = self.extensions.clone();

    if let Some(kind) = self.kind.clone() {
      request.headers_mut().insert(CONTENT_TYPE, kind);
    }

    Multipart::from_request(request, &()).await.map_err(|why| bad(&why.body_text()))
  }
}

impl FromRequest<Served> for Writer {
  type Rejection = Refusal;

  async fn from_request(request: Request, served: &Served) -> Result<Self, Refusal> {
    let headers = request.headers().clone();
    let query = request.uri().query().map(str::to_owned);
    let extensions = request.extensions().clone();
    let (body, form) = if is_form(&headers) {
      (Bytes::new(), Some(request.into_body()))
    } else {
      (Bytes::from_request(request, served).await.map_err(|why| unread(&why))?, None)
    };
    let actor = match named_in(&headers, query.as_deref(), &body)? {
      Some(account) => Actor::Account(account),
      None => Actor::Unnamed,
    };

    Ok(Self {
      actor,
      owners_road: false,
      body,
      form: Mutex::new(form),
      kind: headers.get(CONTENT_TYPE).cloned(),
      extensions,
    })
  }
}

impl FromRequest<Served> for WritableRoom {
  type Rejection = Refusal;

  async fn from_request(request: Request, served: &Served) -> Result<Self, Refusal> {
    let (mut parts, body) = request.into_parts();
    let Path(params) = Path::<HashMap<String, String>>::from_request_parts(&mut parts, served)
      .await
      .map_err(|why| bad(&why.body_text()))?;
    let room = params
      .get("roomId")
      .and_then(|room| room.trim().parse::<i64>().ok())
      .ok_or_else(|| bad("a room is named by its number"))?;
    let writer = Writer::from_request(Request::from_parts(parts, body), served).await?;

    writer.room(&served.receiver, room).await?;

    Ok(Self { room, writer })
  }
}

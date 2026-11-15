//! The one way a write route reaches a room (docs/receiver.md, 「Console rooms and accounts」; docs/api.md, 「Rooms」):
//! every route that changes a room, a message in it or what is kept of it is handed the room through the guard here,
//! which reads who the write acts for off the request and refuses another account's private room before the handler
//! holds anything. Who acts is the account the call names, as `account` or `by` in its query or body or as the relay's
//! `Owner-Key`, all of them one account where several are named; the owner on his own road; or no one, a unit acting
//! as itself under the receiver's token.

use std::collections::HashMap;

use axum::{
  body::{Body, Bytes},
  extract::{FromRequest, FromRequestParts, Multipart, Path, Request},
  http::{HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE},
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
  /// The owner on his own road, `POST /owner/messages` and its kin, which act for his account.
  Owner,
  /// No one: a unit acting as itself under the receiver's token.
  Unit,
}

/// Who a write acts for and what it carries: the one way a write route reads its body, so no route that changes a
/// room reads one without the guard.
pub(super) struct Writer {
  actor: Actor,
  body: Bytes,
  kind: Option<HeaderValue>,
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
      Actor::Owner | Actor::Unit => None,
    }
  }

  /// The body the write carries.
  pub fn body(&self) -> &Bytes {
    &self.body
  }

  /// The write on the owner's own road: for the owner's account where it names no account of its own.
  pub fn owners(mut self) -> Self {
    if self.actor == Actor::Unit {
      self.actor = Actor::Owner;
    }

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
  /// one, for any account; the owner's road writes in his own private room always, and a unit acting as itself is
  /// refused nowhere.
  pub async fn room(&self, receiver: &Receiver, room: i64) -> Result<(), Refusal> {
    let account = match self.actor {
      Actor::Unit => return Ok(()),
      Actor::Owner if room == receiver.owner_room() => return Ok(()),
      Actor::Owner => accounts::owner_account(receiver).await.map_err(reached)?,
      Actor::Account(account) => Some(account),
    };
    let privacy = accounts::privacy(receiver, room).await.map_err(reached)?;
    let allowed = match privacy {
      Privacy::Shared => true,
      Privacy::Of(own) => account == Some(own),
      Privacy::Sealed => false,
    };

    if allowed {
      return Ok(());
    }

    Err(code(&RoomAccountRefusal::PrivateRoom {
      room,
      account: account.unwrap_or_default(),
    }))
  }

  /// The body read again as the multipart form it carries.
  pub async fn form(&self) -> Result<Multipart, Refusal> {
    let mut request = Request::new(Body::from(self.body.clone()));

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
    let body = Bytes::from_request(request, served).await.map_err(|why| bad(&why.body_text()))?;
    let actor = match named_in(&headers, query.as_deref(), &body)? {
      Some(account) => Actor::Account(account),
      None => Actor::Unit,
    };

    Ok(Self {
      actor,
      body,
      kind: headers.get(CONTENT_TYPE).cloned(),
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

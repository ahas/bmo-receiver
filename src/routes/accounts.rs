//! The console's accounts in its rooms (docs/api.md, 「Rooms」 and 「Sends」; docs/receiver.md, 「Console rooms and
//! accounts」): an account's record identity, the rooms it is a member of, a room's members by account let in and
//! taken out with a line saying so, and an account's words and files said in a room, kept under the owner's sender
//! for the owner's account and under the account's record identity for any other. The receiver judges no right of an
//! account here; it holds only that nothing in a private room is said or changed by another account.

use axum::{
  Json,
  extract::{Path, State},
  http::StatusCode,
};
use receiver_codes::{FixedLine, RoomAccountRefusal};
use serde::Deserialize;
use serde_json::{Value, json};

use super::say::{Speaking, files_form, files_said, said, sendable, speakable};
use super::writable::WritableRoom;
use super::{Answered, Refusal, Served, answered, code, read, refused, store};
use crate::accounts::{self, Member, Privacy, Speaker};
use crate::receiver::{NotReceived, Receiver};
use crate::roads::providers::SaidBy;
use crate::routes::say::known;

/// Words an account said in the console.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Words {
  #[serde(default)]
  account: Option<i64>,
  #[serde(default)]
  name: Option<String>,
  text: String,
  #[serde(default)]
  reply_to: Option<Value>,
}

fn reached(why: NotReceived) -> Refusal {
  match why {
    NotReceived::Store(why) => store(why),
    why => refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string()),
  }
}

fn member_shape(member: &Member) -> Value {
  json!({ "account": member.account, "addedBy": member.added_by, "addedAt": member.added_at })
}

// An account's key as the path gives it, decimal text.
fn account_key(account: &str) -> Result<i64, Refusal> {
  account.trim().parse::<i64>().map_err(|_| {
    refused(StatusCode::BAD_REQUEST, "bad-request", "an account is named by its key, a number")
  })
}

// A room the receiver holds, refused 404 where it holds none.
async fn held(receiver: &Receiver, room: i64) -> Result<(), Refusal> {
  if known(receiver, room).await? {
    return Ok(());
  }

  Err(refused(
    StatusCode::NOT_FOUND,
    "not-found",
    &format!("The receiver holds no room {room}."),
  ))
}

// The account a room is private to, where it is a private room.
async fn private_to(receiver: &Receiver, room: i64) -> Result<Option<i64>, Refusal> {
  accounts::private_account(receiver, room).await.map_err(reached)
}

/// `PUT /accounts/{account}/identity`: the account's record identity, made where it does not stand.
pub async fn identity(State(served): State<Served>, Path(account): Path<String>) -> Answered {
  let account = account_key(&account)?;
  let sender = accounts::identity(served.receiver.db(), account).await.map_err(store)?;

  answered(json!({ "account": account, "sender": sender }))
}

/// `GET /accounts/{account}/rooms`: the rooms the account is a member of now, its private rooms among them.
pub async fn rooms(State(served): State<Served>, Path(account): Path<String>) -> Answered {
  let account = account_key(&account)?;
  let receiver: Receiver = (*served.receiver).clone();
  let mut rooms = accounts::private_rooms_of(&receiver, account).await.map_err(reached)?;

  rooms.extend(accounts::rooms_of(receiver.db(), account).await.map_err(store)?);
  rooms.sort_unstable();
  rooms.dedup();

  answered(json!({ "rooms": rooms }))
}

/// `GET /rooms/{roomId}/accounts`: the room's members by account; a private room's own account alone.
pub async fn members(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  held(&receiver, room).await?;

  let members = match private_to(&receiver, room).await? {
    Some(account) => vec![Member {
      account,
      added_by: None,
      added_at: None,
    }],
    None => accounts::members(receiver.db(), room).await.map_err(store)?,
  };

  answered(json!({ "accounts": members.iter().map(member_shape).collect::<Vec<Value>>() }))
}

// A room whose members can be changed: one the receiver holds that is no one's private room, sealed ones among
// them.
async fn changeable(receiver: &Receiver, room: i64, by: i64) -> Result<(), Refusal> {
  held(receiver, room).await?;

  if accounts::privacy(receiver, room).await.map_err(reached)? != Privacy::Shared {
    return Err(code(&RoomAccountRefusal::PrivateRoom { room, account: by }));
  }

  Ok(())
}

// A line the room hears of a member's change, said by the receiver itself; a room that takes no line is said in the
// journal instead, and the change stands either way.
async fn told(receiver: &Receiver, room: i64, line: FixedLine) {
  let Some(text) = receiver.speech().rendered(&line).await else { return };

  if let Err(why) = receiver.speech().say(room, &text).await {
    println!("chat {room}: a member's change went unsaid: {why}");
  }
}

/// `PUT /rooms/{roomId}/accounts/{account}`: an account let in by `by`, with a line saying so; the member as it
/// stands where it already is one.
pub async fn let_in(
  State(served): State<Served>,
  Path((_, account)): Path<(i64, String)>,
  writable: WritableRoom,
) -> Answered {
  let room = writable.room;
  let account = account_key(&account)?;
  let Some(by) = writable.writer.account() else {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "by (an account) is required"));
  };
  let receiver: Receiver = (*served.receiver).clone();

  changeable(&receiver, room, by).await?;

  let (member, now) = accounts::let_in(receiver.db(), room, account, by).await.map_err(store)?;

  if now {
    told(&receiver, room, FixedLine::AccountLetIn { account, by }).await;
  }

  answered(member_shape(&member))
}

/// `DELETE /rooms/{roomId}/accounts/{account}?by=`: a member taken out by `by`, leaving where `by` is itself, with a
/// line saying so; nothing where it is no member.
pub async fn take_out(
  State(served): State<Served>,
  Path((_, account)): Path<(i64, String)>,
  writable: WritableRoom,
) -> Result<StatusCode, Refusal> {
  let room = writable.room;
  let account = account_key(&account)?;
  let Some(by) = writable.writer.account() else {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "by (an account) is required"));
  };
  let receiver: Receiver = (*served.receiver).clone();

  changeable(&receiver, room, by).await?;

  if accounts::take_out(receiver.db(), room, account, by).await.map_err(store)? {
    let line = if by == account {
      FixedLine::AccountLeft { account }
    } else {
      FixedLine::AccountTakenOut { account, by }
    };

    told(&receiver, room, line).await;
  }

  Ok(StatusCode::NO_CONTENT)
}

/// Who an account speaks as in a room: the owner on his own sender for the owner's account, and the account under
/// its record identity and the shown name the console gives for any other.
pub(super) async fn speaker(
  receiver: &Receiver,
  room: i64,
  account: i64,
  name: &str,
) -> Result<Speaking, Refusal> {
  Ok(match accounts::speaker(receiver, account).await.map_err(reached)? {
    Speaker::Owner => Speaking::owner(receiver, room).await,
    Speaker::Account { sender } => Speaking {
      sender_id: sender,
      name: name.to_owned(),
      said_by: SaidBy::Account {
        account,
        name: name.to_owned(),
      },
    },
  })
}

// The account and its shown name a call names, refused where either is missing.
fn named(account: Option<i64>, name: Option<&str>) -> Result<(i64, String), Refusal> {
  let name = name.map(str::trim).filter(|name| !name.is_empty());

  match (account, name) {
    (Some(account), Some(name)) => Ok((account, name.to_owned())),
    _ => Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      "account (a number) and name (a non-empty string) are required",
    )),
  }
}

/// `POST /rooms/{roomId}/words`: words an account said in the console, kept and carried as any message.
pub async fn words(
  State(served): State<Served>,
  writable: WritableRoom,
) -> Result<(StatusCode, Json<Value>), Refusal> {
  let room = writable.room;
  let asked = read::<Words>(writable.writer.body())?;
  let (account, name) = named(asked.account, asked.name.as_deref())?;
  let receiver: Receiver = (*served.receiver).clone();
  let text = speakable(&receiver, room, &asked.text).await?;
  let speaking = speaker(&receiver, room, account, &name).await?;
  let id = said(&receiver, room, &text, asked.reply_to.as_ref(), speaking).await?;

  Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

/// `POST /rooms/{roomId}/word-files`: files an account sent in the console, as `POST /owner/message-files` takes
/// them, kept and carried as any message.
pub async fn word_files(State(served): State<Served>, writable: WritableRoom) -> Answered {
  let room = writable.room;
  let receiver: Receiver = (*served.receiver).clone();
  let form = files_form(writable.writer.form().await?).await?;
  let (account, name) = named(form.account, form.name.as_deref())?;

  // The form names the account the request could not read off it, so the room is asked again for that account.
  writable.writer.for_account(Some(account))?.room(&receiver, room).await?;
  sendable(&receiver, room, &form).await?;

  let speaking = speaker(&receiver, room, account, &name).await?;

  files_said(&receiver, room, form, speaking).await
}

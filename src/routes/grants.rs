//! A room member's rights over its people, stood beside its membership (`docs/receiver.md`, 「Console rooms and
//! accounts」): which members may let a person in and take a member out. A manager who governs the room writes them,
//! and the web unit's entry check reads them beside the room's leaders and members. The receiver keeps the rights and
//! judges who may write them — its administrator, the room's leader or a head of a leader's unit, the same standing
//! that locks a room — and holds that nothing in a private room is written by another account.

use axum::{
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use serde::Deserialize;
use serde_json::json;

use super::accounts::{account_key, by_of, held};
use super::writable::WritableRoom;
use super::{Answered, Refusal, Served, answered, governs_room, query, read, refused, store};
use crate::accounts::{self, Right};
use crate::receiver::Receiver;

/// What `GET` asks beside the path: the asking account, whose standing says whether it may write these rights.
#[derive(Deserialize)]
struct Asked {
  by: String,
}

/// What `PUT` carries beside `by`: the one right to grant.
#[derive(Deserialize)]
struct Granting {
  right: String,
}

/// What `DELETE` asks beside `by`: the one right to take back.
#[derive(Deserialize)]
struct Revoking {
  right: String,
}

// One right, named as the route words it; anything else is refused.
fn right_of(text: &str) -> Result<Right, Refusal> {
  match text.trim() {
    "let-in" => Ok(Right::LetIn),
    "take-out" => Ok(Right::TakeOut),
    _ => Err(refused(StatusCode::BAD_REQUEST, "bad-request", "right is `let-in` or `take-out`")),
  }
}

// A member's rights are written by whoever governs the room, refused as a room's archive is.
async fn governed(receiver: &Receiver, by: i64, room: i64) -> Result<(), Refusal> {
  if governs_room(receiver, &by.to_string(), room).await? {
    return Ok(());
  }

  Err(refused(
    StatusCode::FORBIDDEN,
    "not-room-leader",
    "a member's rights are written by the administrator, by the room's leader or by a head of their unit; ask one of them",
  ))
}

/// `GET /rooms/{roomId}/accounts/{account}/grants?by=`: one member's rights in the room, and whether the asking
/// account may write them (`docs/api.md`, 「Rooms」).
pub async fn read_grants(
  State(served): State<Served>,
  Path((room, account)): Path<(i64, String)>,
  raw: RawQuery,
) -> Answered {
  let account = account_key(&account)?;
  let asked = query::<Asked>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();

  held(&receiver, room).await?;

  let standing = accounts::grants(receiver.db(), room, account).await.map_err(store)?;
  let may_write = governs_room(&receiver, asked.by.trim(), room).await?;

  answered(json!({
    "account": account,
    "letIn": standing.let_in,
    "takeOut": standing.take_out,
    "mayWrite": may_write,
  }))
}

/// `PUT /rooms/{roomId}/accounts/{account}/grants`: one of a member's rights granted by a manager, the member's
/// rights as they now stand; granting a right the member holds already changes nothing.
pub async fn grant_right(
  State(served): State<Served>,
  Path((_, account)): Path<(i64, String)>,
  writable: WritableRoom,
) -> Answered {
  let room = writable.room;
  let account = account_key(&account)?;
  let by = by_of(&writable)?;
  let asked = read::<Granting>(writable.writer.body())?;
  let right = right_of(&asked.right)?;
  let receiver: Receiver = (*served.receiver).clone();

  held(&receiver, room).await?;
  governed(&receiver, by, room).await?;

  let standing = accounts::grant(receiver.db(), room, account, right).await.map_err(store)?;

  answered(json!({
    "account": account,
    "letIn": standing.let_in,
    "takeOut": standing.take_out,
  }))
}

/// `DELETE /rooms/{roomId}/accounts/{account}/grants?by=&right=`: one of a member's rights taken back by a manager;
/// answered whether the member held it, for the rights stand the same either way.
pub async fn take_back(
  State(served): State<Served>,
  Path((_, account)): Path<(i64, String)>,
  raw: RawQuery,
  writable: WritableRoom,
) -> Result<StatusCode, Refusal> {
  let room = writable.room;
  let account = account_key(&account)?;
  let by = by_of(&writable)?;
  let asked = query::<Revoking>(raw)?;
  let right = right_of(&asked.right)?;
  let receiver: Receiver = (*served.receiver).clone();

  held(&receiver, room).await?;
  governed(&receiver, by, room).await?;

  accounts::revoke(receiver.db(), room, account, right).await.map_err(store)?;

  Ok(StatusCode::NO_CONTENT)
}

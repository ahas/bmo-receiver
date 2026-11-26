//! Whether the receiver listens to a room (docs/wake.md, 「Who reaches the session」; docs/store.md,
//! `allowed_chats`): the rooms the console's room list holds, the owner's private room always among them. A room
//! the receiver does not listen to is kept all the same, and wakes nobody.

use sea_orm::DatabaseConnection;

use super::pinned;
use crate::{Fault, one};

/// What puts a room on the console's room list, each read off the receiver's own record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Standing {
  /// The owner's private room.
  pub owner_room: bool,
  /// On the listen list: the owner allowed it with his allow command and has not denied it since.
  pub allowed: bool,
  /// Standing in a mode, an addition's or one the administrator set on it since.
  pub in_mode: bool,
  /// Naming the supervisors that answer it.
  pub supervised: bool,
  /// Pinned over the rest.
  pub pinned: bool,
}

impl Standing {
  /// Whether the room stands on the console's room list, and so is listened to.
  pub fn listened(self) -> bool {
    self.owner_room || self.allowed || self.in_mode || self.supervised || self.pinned
  }

  /// What the record says of one room, `owner_room` being the deployment's private room.
  pub async fn of(db: &DatabaseConnection, owner_room: i64, chat_id: i64) -> Result<Self, Fault> {
    let held = |table: &'static str| async move {
      Ok::<bool, Fault>(
        one(
          db,
          &format!(r#"SELECT 1 AS "held" FROM "{table}" WHERE "chat_id" = $1 LIMIT 1"#),
          vec![chat_id.into()],
        )
        .await?
        .is_some(),
      )
    };

    Ok(Self {
      owner_room: chat_id == owner_room,
      allowed: held("allowed_chats").await?,
      in_mode: held("room_modes").await?,
      supervised: held("room_supervisors").await?,
      pinned: pinned::pinned(db).await?.contains(&chat_id),
    })
  }
}

/// Whether the receiver listens to a room, `owner_room` being the deployment's private room.
pub async fn listens(db: &DatabaseConnection, owner_room: i64, chat_id: i64) -> Result<bool, Fault> {
  if chat_id == owner_room {
    return Ok(true);
  }

  Ok(Standing::of(db, owner_room, chat_id).await?.listened())
}

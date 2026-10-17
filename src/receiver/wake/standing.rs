//! What the room's own record and the account model say of who is heard there: the reply mode, a mute, and
//! whether a standing rule of the room names the sender (docs/rooms.md, 「Delegations」).

use receiver_ports::web::RoomCheck;

use super::asked;
use crate::one;
use crate::receiver::{NotReceived, Receiver};
use crate::store::settings;

// The words a room rule is held by, which make a person the owner's delegate there.
const RULE: &str = "rule";

/// Whether everyone is heard, rather than the owner alone, which is the mode with nothing set.
pub async fn hears_everyone(receiver: &Receiver) -> Result<bool, NotReceived> {
  Ok(settings::get(receiver.db(), "mode").await?.as_deref() == Some("everyone"))
}

/// Whether the owner muted a person in a room.
pub async fn is_muted(receiver: &Receiver, chat_id: i64, sender: i64) -> Result<bool, NotReceived> {
  Ok(
    one(
      receiver.db(),
      r#"SELECT 1 AS "one" FROM "muted_members" WHERE "chat_id" = $1 AND "user_id" = $2"#,
      vec![chat_id.into(), sender.into()],
    )
    .await?
    .is_some(),
  )
}

/// Whether a standing rule of the room names the sender: a grant of the room's rule to the user the sender is,
/// still in force, as the account model says. Such a person is the owner's delegate there: heard in owner-only
/// mode, and their file reaches the chat agent uncalled.
pub async fn holds_rule(
  receiver: &Receiver,
  chat_id: i64,
  sender: Option<i64>,
) -> Result<bool, NotReceived> {
  let Some(sender) = sender else {
    return Ok(false);
  };
  let check = RoomCheck {
    room: chat_id,
    sender: Some(sender),
    relation: RULE.to_owned(),
  };

  Ok(
    asked("whether the room's rule is held", receiver.units.web.check_room(&check).await)
      .is_some_and(|held| held.allowed),
  )
}

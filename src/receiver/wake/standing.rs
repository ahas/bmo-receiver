//! What the room's own record says of who is heard there: a mute (docs/rooms.md).

use crate::one;
use crate::receiver::{NotReceived, Receiver};

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

//! An answer wanted back from another room (docs/messages.md, 「Words handed to the room's supervisor」): a line
//! handed to a room with `reply` from a round of another room waits on the order it rides, and is armed only once
//! the round carrying that order has sent its message, so the next answer of that room is relayed to the room that
//! asked. A round that sends nothing arms nothing, and the row lapses with its life.

use sea_orm::DatabaseConnection;

use crate::{Fault, now_ms, one, run};

/// How long the room has to answer before the promise lapses, counted from the moment it is armed.
const RELAY_LIFE_MS: i64 = 30 * 60_000;

// The brake on two rooms answering each other forever: a room may ask other rooms for at most this many answers
// in this stretch, and the ask beyond it is dropped while the words themselves are still handed.
const MAX_RELAYS_ASKED: i64 = 3;
const RELAY_CAP_WINDOW_MS: i64 = 10 * 60_000;

// The first line of what was handed is named by at most this many characters.
const OPENING_WIDTH: usize = 60;

/// Whether an answer asked for will be brought back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relay {
  Waiting,
  Dropped,
}

/// The first line of the words, short enough to name the message without repeating it.
pub fn opening(text: &str) -> String {
  let first = text.trim().split('\n').next().unwrap_or_default();

  if first.chars().count() <= OPENING_WIDTH {
    first.to_owned()
  } else {
    format!("{}…", first.chars().take(OPENING_WIDTH).collect::<String>())
  }
}

/// The answer of `chat_id` wanted back in `asking`, waiting on the handed order `order` that carries the words;
/// dropped where `asking` has already asked the most rooms within the stretch.
pub async fn wait_on_order(
  db: &DatabaseConnection,
  chat_id: i64,
  asking: i64,
  order: i64,
  text: &str,
) -> Result<Relay, Fault> {
  let now = now_ms();
  let asked: i64 = one(
    db,
    r#"SELECT COUNT(*) AS "n" FROM "say_relays" WHERE "reply_to" = $1 AND "created_at" >= $2"#,
    vec![asking.into(), (now - RELAY_CAP_WINDOW_MS).into()],
  )
  .await?
  .map(|row| row.try_get("", "n"))
  .transpose()
  .map_err(Fault::from)?
  .unwrap_or(0);

  if asked >= MAX_RELAYS_ASKED {
    return Ok(Relay::Dropped);
  }

  run(
    db,
    r#"INSERT INTO "say_relays" ("chat_id", "reply_to", "opening", "created_at", "expires_at", "order_id")
       VALUES ($1, $2, $3, $4, $5, $6)"#,
    vec![
      chat_id.into(),
      asking.into(),
      opening(text).into(),
      now.into(),
      (now + RELAY_LIFE_MS).into(),
      order.into(),
    ],
  )
  .await?;

  Ok(Relay::Waiting)
}

/// The relays of `chat_id` waiting on any of `orders` armed, now that the round carrying them has sent its
/// message: from here the room's next answer goes back to the room that asked.
pub async fn arm(db: &DatabaseConnection, chat_id: i64, orders: &[i64]) -> Result<u64, Fault> {
  if orders.is_empty() {
    return Ok(0);
  }

  let now = now_ms();
  let listed = orders.iter().map(i64::to_string).collect::<Vec<_>>().join(", ");

  run(
    db,
    &format!(
      r#"UPDATE "say_relays" SET "order_id" = NULL, "created_at" = $2, "expires_at" = $3
         WHERE "chat_id" = $1 AND "order_id" IN ({listed})"#
    ),
    vec![chat_id.into(), now.into(), (now + RELAY_LIFE_MS).into()],
  )
  .await
}

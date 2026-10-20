//! A board of running jobs left pinned in the owner's chat (docs/store.md, 「The settings」): nothing draws one, and
//! the database may still hold the numbers of one a process put up there. As the receiver starts, each is taken out
//! of the chat on its road — deleted, or unpinned where the road will not delete it — and the numbers are forgotten
//! once each is gone. A number whose take-down failed any other way is kept for the next look.

use bmo_chat_client::{ChatSend, ChatSendRequest};
use serde_json::Value;

use super::Receiver;
use super::speech::NotSaid;
use crate::store::settings;

// The settings the numbers stand under: every board's, oldest first, and the single number kept before there was a
// list.
const LEDGER_KEY: &str = "job_board_messages";
const LEGACY_KEY: &str = "job_board_message_id";

/// Every board the database still holds a number for taken out of the owner's chat, on the road the chat is on.
pub async fn take_down(receiver: &Receiver) {
  let db = receiver.db();
  let ledger = settings::get(db, LEDGER_KEY).await.ok().flatten();
  let legacy = settings::get(db, LEGACY_KEY).await.ok().flatten();

  if ledger.is_none() && legacy.is_none() {
    return;
  }

  let mut numbers: Vec<i64> = ledger
    .and_then(|raw| serde_json::from_str::<Vec<Value>>(&raw).ok())
    .map(|ids| ids.iter().filter_map(Value::as_i64).collect())
    .unwrap_or_default();

  numbers.extend(legacy.and_then(|value| value.trim().parse::<i64>().ok()));
  numbers.sort_unstable();
  numbers.dedup();

  let owner = receiver.owner_id();
  let road = receiver.roads().room_road(owner).await;
  let mut standing = Vec::new();

  for message_id in numbers {
    let out = match &road {
      Some(road) => taken_out(receiver, road, owner, message_id).await,
      None => false,
    };

    if !out {
      standing.push(message_id);
    }
  }

  let written = if standing.is_empty() {
    settings::forget(db, LEDGER_KEY).await
  } else {
    settings::set(db, LEDGER_KEY, &serde_json::to_string(&standing).unwrap_or_default()).await
  };

  if let Err(why) = written.and(settings::forget(db, LEGACY_KEY).await) {
    eprintln!("bmo-receiver: the numbers of the board left in the owner's chat were not written: {why}");
  }
}

// One board out of the chat: deleted, or unpinned where the road refuses the deletion — a board older than the
// platform lets the account delete — whether it is out. The road's answer to the unpinning is the last word on it; a
// failure to reach the road at all leaves the board for the next look.
async fn taken_out(receiver: &Receiver, road: &str, chat_id: i64, message_id: i64) -> bool {
  let speech = receiver.speech();
  let delete = ChatSendRequest::new(chat_id, ChatSend::Delete { message_id });

  match speech.carried(road, &delete).await {
    Ok(_) => return true,
    Err(NotSaid::Refused { .. }) => {}
    Err(why) => {
      eprintln!("bmo-receiver: the board {message_id} left in the owner's chat was not deleted: {why}");
      return false;
    }
  }

  let unpin = ChatSendRequest::new(
    chat_id,
    ChatSend::Pin {
      message_id,
      on: false,
      silent: true,
    },
  );

  match speech.carried(road, &unpin).await {
    Ok(_) => true,
    Err(NotSaid::Refused { refusal, .. }) => {
      eprintln!(
        "bmo-receiver: the board {message_id} left in the owner's chat was neither deleted nor unpinned: {refusal}"
      );
      true
    }
    Err(why) => {
      eprintln!("bmo-receiver: the board {message_id} left in the owner's chat was not unpinned: {why}");
      false
    }
  }
}

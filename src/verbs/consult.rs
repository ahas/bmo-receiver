//! The consult verbs (docs/consult.md, 「The consult tool」): the consult room read or set by number or title,
//! and the block an agent is shown of the other agents' work as it would read it now. A line said in the room is
//! carried out by the running service through `POST /consult/say`, not here.

use receiver_policy::consult::CONSULT;
use receiver_policy::fill_marks;

use crate::companion::consult::{consult_block, window_states};
use crate::receiver::{Receiver, rows};
use crate::store::settings;
use crate::{all, now_ms};

// A room as a verb shows it: its title, or its number where it has none, and its number.
pub async fn shown(receiver: &Receiver, chat_id: i64, line: &str) -> String {
  let chat = chat_id.to_string();
  let title = rows::chat_title(receiver.db(), chat_id).await.ok().flatten().unwrap_or(chat.clone());

  fill_marks(line, &[("title", &title), ("chat", &chat)])
}

/// The consult room as it is set, or with a room named — by number or title — set anew to it, where at least
/// two designated agents answer there; any other room is refused.
pub async fn room(receiver: &Receiver, named: Option<&str>) -> Result<Vec<String>, String> {
  let db = receiver.db();

  let Some(named) = named else {
    let set = settings::get(db, &CONSULT.room_setting)
      .await
      .ok()
      .flatten()
      .and_then(|stored| stored.trim().parse::<i64>().ok());

    return Ok(vec![match set {
      Some(chat_id) => shown(receiver, chat_id, &CONSULT.room_shown).await,
      None => CONSULT.room_unset.clone(),
    }]);
  };
  let wanted = named.trim();
  let rooms = all(db, r#"SELECT DISTINCT "chat_id" FROM "room_answerers" ORDER BY "chat_id""#, vec![])
    .await
    .ok()
    .unwrap_or_default();
  let numbered: Vec<i64> = rooms.iter().filter_map(|row| row.try_get::<i64>("", "chat_id").ok()).collect();
  let mut named_rooms: Vec<(i64, String)> = vec![];

  for chat_id in numbered {
    named_rooms.push((chat_id, rows::chat_title(db, chat_id).await.ok().flatten().unwrap_or_default()));
  }

  let Some(chat_id) = named_rooms
    .iter()
    .find(|(chat_id, title)| chat_id.to_string() == wanted || title == wanted)
    .map(|(chat_id, _)| *chat_id)
  else {
    return Err(fill_marks(&CONSULT.not_answered, &[("room", wanted)]));
  };

  if window_states(receiver, chat_id).await.len() < 2 {
    return Err(fill_marks(&CONSULT.not_shared, &[("room", wanted)]));
  }

  if let Some(why) = settings::set(db, &CONSULT.room_setting, &chat_id.to_string()).await.err() {
    return Err(why.detail);
  }

  Ok(vec![shown(receiver, chat_id, &CONSULT.room_chosen).await])
}

/// The block an agent is shown of the other agents' work, as it would read it now.
pub async fn state(receiver: &Receiver, agent: &str) -> Vec<String> {
  let block = consult_block(receiver, agent, now_ms() / 1000).await.ok().flatten();

  vec![block.unwrap_or_else(|| fill_marks(&CONSULT.nothing_shown, &[("agent", agent)]))]
}

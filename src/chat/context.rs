//! A turn's prompt opened on its way out (the gate's reference selection; docs/unified-
//! bmo.md, 「The system prompt and the room switch」): each reference document wrapped once goes in full, or as
//! the line saying the session already holds that version; the room's block goes in full, by its mark where the
//! session holds this version from earlier, or not at all where the session read this very block last; and once
//! the turn is answered, what the session that took it now holds is written down under it.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::js::bun_hash;
use regex::Regex;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use super::room::RoomContext;
use crate::{Fault, all, now_ms, one, run};

// The wrapping a document rides in until the session it goes to is settled (`window::once_block`).
const BLOCK_OPEN: &str = "\u{e000}REFERENCE-ONCE ";

static BLOCK: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"\x{e000}REFERENCE-ONCE (\{[^\n]*\})\n((?s:.*?))\n\x{e000}END REFERENCE-ONCE")
    .expect("a wrapped document")
});

#[derive(Deserialize)]
struct Head {
  path: String,
  chat: i64,
  version: String,
  // Read only so a head naming a person by anything but a number is no head.
  #[serde(default, rename = "person")]
  _person: Option<i64>,
}

/// How a turn restates its room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomSend {
  None,
  Mark,
  Full,
}

/// How a document of the turn went in, as the turn's log says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
  Held,
  Attached,
}

/// The version of a room's block: the block itself, whatever the mark beside it says.
pub fn fingerprint(room: &RoomContext) -> String {
  #[derive(Serialize)]
  #[serde(rename_all = "camelCase")]
  struct Versioned<'a> {
    chat_id: i64,
    text: &'a str,
  }

  bun_hash(
    &serde_json::to_string(&Versioned {
      chat_id: room.chat_id,
      text: &room.text,
    })
    .unwrap_or_default(),
  )
  .to_string()
}

// Where a room's block stands in the session's record of what it was sent in full.
fn room_path(chat_id: i64) -> String {
  format!("room-context/{chat_id}")
}

/// What a session holds of what was sent in full: each path with the version it holds.
pub async fn loaded_in(
  db: &DatabaseConnection,
  session: Option<&str>,
) -> Result<HashMap<String, String>, Fault> {
  let Some(session) = session else { return Ok(HashMap::new()) };

  Ok(
    all(
      db,
      r#"SELECT "path", "version" FROM "session_people" WHERE "session_id" = $1"#,
      vec![session.into()],
    )
    .await?
    .iter()
    .filter_map(|row| Some((row.try_get("", "path").ok()?, row.try_get("", "version").ok()?)))
    .collect(),
  )
}

/// How the turn restates its room: not at all where the session read this very block last, by its mark where it
/// holds this version from earlier, in full otherwise. A turn that continues no session holds nothing.
pub async fn room_send(
  db: &DatabaseConnection,
  session: Option<&str>,
  room: &RoomContext,
) -> Result<RoomSend, Fault> {
  let Some(session) = session else { return Ok(RoomSend::Full) };
  let version = fingerprint(room);

  let read = one(
    db,
    r#"SELECT "fingerprint" FROM "bmo_room_contexts" WHERE "session_id" = $1"#,
    vec![session.into()],
  )
  .await?
  .and_then(|row| row.try_get::<String>("", "fingerprint").ok());

  if read.as_deref() == Some(version.as_str()) {
    return Ok(RoomSend::None);
  }

  Ok(if loaded_in(db, Some(session)).await?.get(&room_path(room.chat_id)) == Some(&version) {
    RoomSend::Mark
  } else {
    RoomSend::Full
  })
}

// Each wrapped document of a prompt, and whether the session already holds that very version.
fn once_entries(prompt: &str, loaded: &HashMap<String, String>) -> Vec<(Head, bool)> {
  if !prompt.contains(BLOCK_OPEN) {
    return vec![];
  }

  BLOCK
    .captures_iter(prompt)
    .filter_map(|found| serde_json::from_str::<Head>(&found[1]).ok())
    .map(|head| {
      let held = loaded.get(&head.path) == Some(&head.version);

      (head, held)
    })
    .collect()
}

/// Each wrapped document opened: its text where the session does not hold its version, the line saying it does
/// where it does. Only the body gives way: the heading names the path.
pub fn opened(prompt: &str, loaded: &HashMap<String, String>) -> String {
  if !prompt.contains(BLOCK_OPEN) {
    return prompt.to_owned();
  }

  let mut sent: Vec<(String, bool, i64)> = vec![];
  let text = BLOCK
    .replace_all(prompt, |found: &regex::Captures| match serde_json::from_str::<Head>(&found[1]) {
      Ok(head) => {
        let held = loaded.get(&head.path) == Some(&head.version);

        sent.push((head.path.clone(), held, head.chat));

        if held { format!("[{}] (already loaded)", head.path) } else { found[2].to_owned() }
      }
      Err(_) => found[2].to_owned(),
    })
    .into_owned();

  if let Some((_, _, chat)) = sent.first() {
    let paths = |held: bool| {
      let named: Vec<&str> = sent.iter().filter(|one| one.1 == held).map(|one| one.0.as_str()).collect();

      if named.is_empty() { "none".to_owned() } else { named.join(", ") }
    };

    println!("reference: chat {chat} in full: {}; already loaded: {}", paths(false), paths(true));
  }

  text
}

/// What the turn was handed, each reference document marked with how it goes in: the same comparison the
/// prompt is opened by.
pub fn states(prompt: &str, loaded: &HashMap<String, String>) -> HashMap<String, State> {
  once_entries(prompt, loaded)
    .into_iter()
    .map(|(head, held)| (head.path, if held { State::Held } else { State::Attached }))
    .collect()
}

/// The prompt a turn goes out with: each wrapped document opened, and the room restated as the session needs it.
/// A turn whose room is restated has the session's record of the block let go first, since a turn that fails may
/// still have taken the new room.
pub async fn sent_prompt(
  db: &DatabaseConnection,
  session: Option<&str>,
  room: Option<&RoomContext>,
  restates: bool,
  prompt: &str,
) -> Result<(String, Option<RoomSend>), Fault> {
  let prompt = opened(prompt, &loaded_in(db, session).await?);
  let Some(room) = room.filter(|_| restates) else { return Ok((prompt, None)) };
  let send = room_send(db, session, room).await?;

  match send {
    RoomSend::None => Ok((prompt, Some(send))),
    RoomSend::Mark | RoomSend::Full => {
      forget_room(db, session).await?;

      let said = if send == RoomSend::Full { &room.text } else { &room.mark };

      Ok((format!("{said}\n\n{prompt}"), Some(send)))
    }
  }
}

/// The session's record of the room's block let go, so the next turn restates it.
pub async fn forget_room(db: &DatabaseConnection, session: Option<&str>) -> Result<(), Fault> {
  if let Some(session) = session {
    run(db, r#"DELETE FROM "bmo_room_contexts" WHERE "session_id" = $1"#, vec![session.into()]).await?;
  }

  Ok(())
}

/// Everything sent once let go of a session that folds: it keeps its name and loses the detail.
pub async fn forget_documents(db: &DatabaseConnection, session: Option<&str>) -> Result<(), Fault> {
  if let Some(session) = session {
    run(db, r#"DELETE FROM "session_people" WHERE "session_id" = $1"#, vec![session.into()]).await?;
  }

  Ok(())
}

/// What a session now holds of what the turn was built with, written under the session that took it: the room's
/// block where it went in full or the session it continued took it, the room as one it has seen, and every
/// reference document that went in full. `prompt` is the turn's prompt before it was opened.
pub async fn remember_held(
  db: &DatabaseConnection,
  resumed: Option<&str>,
  session: &str,
  room: Option<&RoomContext>,
  restates: bool,
  prompt: &str,
) -> Result<(), Fault> {
  let mut loaded: Vec<(String, String)> = vec![];

  if let Some(room) = room.filter(|_| restates) {
    // Asked before anything below is written, it answers what the send decided.
    if room_send(db, resumed, room).await? == RoomSend::Full || Some(session) == resumed {
      let version = fingerprint(room);

      run(
        db,
        r#"INSERT INTO "bmo_room_contexts" VALUES ($1, $2) ON CONFLICT ("session_id") DO UPDATE SET "fingerprint" = excluded."fingerprint""#,
        vec![session.into(), version.clone().into()],
      )
      .await?;

      loaded.push((room_path(room.chat_id), version));
    }

    run(
      db,
      r#"INSERT INTO "bmo_session_rooms" VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
      vec![session.into(), room.chat_id.into()],
    )
    .await?;
  }

  let held = loaded_in(db, resumed).await?;

  loaded.extend(
    once_entries(prompt, &held)
      .into_iter()
      .filter(|(_, held)| !held)
      .map(|(head, _)| (head.path, head.version)),
  );

  if loaded.is_empty() {
    return Ok(());
  }

  let at = now_ms() / 1000;

  for (path, version) in loaded {
    run(
      db,
      r#"INSERT INTO "session_people" ("session_id", "path", "version", "loaded_at") VALUES ($1, $2, $3, $4)
         ON CONFLICT ("session_id", "path") DO UPDATE SET "version" = excluded."version", "loaded_at" = excluded."loaded_at""#,
      vec![session.into(), path.into(), version.into(), at.into()],
    )
    .await?;
  }

  // Rows of sessions untouched for a week go at the same stroke: a session that old has long been replaced.
  run(
    db,
    r#"DELETE FROM "session_people" WHERE "session_id" IN (SELECT "session_id" FROM "session_people" GROUP BY "session_id" HAVING MAX("loaded_at") < $1)"#,
    vec![(at - 7 * 86_400).into()],
  )
  .await?;

  Ok(())
}

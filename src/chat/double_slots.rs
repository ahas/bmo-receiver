//! The double's slots the receiver holds (docs/drafting.md, 「A round handed to the double」): a slot's key is kept
//! before the slot is asked for and dropped once the agency took it back, so `quiet` counts the double's rounds
//! still under way and the start gives back, by the key it kept, every slot a stopped process left.

use receiver_ports::agency::Agency;
use sea_orm::DatabaseConnection;

use crate::{Fault, all, one, run};

/// A slot the receiver holds: its key, the agent and the room whose round it carries, and when it was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
  pub key: String,
  pub agent: String,
  pub chat_id: i64,
  pub asked_at: i64,
}

/// The slot kept as held before it is asked for, so a process that stops at any step after leaves it to be given
/// back.
pub async fn keep(db: &DatabaseConnection, held: &Held) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "double_slots" ("key", "agent", "chat_id", "asked_at") VALUES ($1, $2, $3, $4)
       ON CONFLICT ("key") DO NOTHING"#,
    vec![
      held.key.clone().into(),
      held.agent.clone().into(),
      held.chat_id.into(),
      held.asked_at.into(),
    ],
  )
  .await?;

  Ok(())
}

/// The slot no longer held: the agency took it back.
pub async fn given_back(db: &DatabaseConnection, key: &str) -> Result<(), Fault> {
  run(db, r#"DELETE FROM "double_slots" WHERE "key" = $1"#, vec![key.into()]).await?;

  Ok(())
}

/// Every slot held, oldest first; none in a store not yet migrated to hold them, as `quiet` reads it from the
/// release about to go live against the store the running one keeps.
pub async fn held(db: &DatabaseConnection) -> Result<Vec<Held>, Fault> {
  let kept = one(db, r#"SELECT to_regclass('double_slots') IS NOT NULL AS "kept""#, vec![])
    .await?
    .and_then(|row| row.try_get::<bool>("", "kept").ok())
    .unwrap_or(false);

  if !kept {
    return Ok(vec![]);
  }

  let rows = all(
    db,
    r#"SELECT "key", "agent", "chat_id", "asked_at" FROM "double_slots" ORDER BY "asked_at", "key""#,
    vec![],
  )
  .await?;

  Ok(
    rows
      .iter()
      .filter_map(|row| {
        Some(Held {
          key: row.try_get("", "key").ok()?,
          agent: row.try_get("", "agent").ok()?,
          chat_id: row.try_get("", "chat_id").ok()?,
          asked_at: row.try_get("", "asked_at").ok()?,
        })
      })
      .collect(),
  )
}

/// Every slot held, each named as a line of work under way.
pub async fn under_way(db: &DatabaseConnection) -> Result<Vec<String>, Fault> {
  Ok(
    held(db)
      .await?
      .into_iter()
      .map(|held| {
        format!(
          "double's round {} of {} in chat {} under way since {}",
          held.key, held.agent, held.chat_id, held.asked_at
        )
      })
      .collect(),
  )
}

/// The slot given back by its key, with the role session bound to it, and no longer held once the agency took it;
/// still held, to be given back at the next start, where the agency did not answer.
pub async fn release(db: &DatabaseConnection, agency: &dyn Agency, key: &str) -> bool {
  if let Err(why) = agency.double_release(key).await {
    eprintln!("double's slot {key}: not given back: {why}");

    return false;
  }

  if let Err(why) = given_back(db, key).await {
    eprintln!("double's slot {key}: given back, but still kept as held: {}", why.detail);

    return false;
  }

  true
}

/// Every slot asked before `before` given back: no round of this process follows it, so its double's run is
/// stopped rather than left beside the agent; the rows its turn carried wait again as the restart left them.
pub async fn release_left(db: &DatabaseConnection, agency: &dyn Agency, before: i64) {
  let left = match held(db).await {
    Ok(left) => left,
    Err(why) => {
      eprintln!("double's slots: the slots a stopped process held were not read: {}", why.detail);

      return;
    }
  };

  for held in left.into_iter().filter(|held| held.asked_at < before) {
    if release(db, agency, &held.key).await {
      println!(
        "double's slot {}: left by a stopped process for {} in chat {}, given back",
        held.key, held.agent, held.chat_id
      );
    }
  }
}

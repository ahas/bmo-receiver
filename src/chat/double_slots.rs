//! The slots of the agency's double role a store still keeps (docs/receiver.md, 「Where it stands」): their keys
//! are read as the receiver starts, before the migration that drops their table, and each is given back once by
//! the agency's owner release, so nothing stays held at the agency for a round no process follows. `quiet` counts
//! each as a round under way while the store it reads still keeps them (docs/boundary-api.md, `quiet`).

use receiver_ports::agency::Agency;
use sea_orm::DatabaseConnection;

use crate::{Fault, all, one};

/// A slot the store keeps: its key, the agent and the room whose round it carried, and when it was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
  pub key: String,
  pub agent: String,
  pub chat_id: i64,
  pub asked_at: i64,
}

/// Every slot the store keeps, oldest first; none in a store without their table.
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

/// Every slot the store keeps, each named as a line of work under way.
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

/// Every slot read before the store's migration given back once by its key, with the role session bound to it;
/// a key the agency does not take is named in the log and not kept, since its table is gone.
pub async fn give_back(agency: &dyn Agency, left: &[Held]) {
  for held in left {
    match agency.double_release(&held.key).await {
      Ok(()) => println!(
        "double's slot {}: left for {} in chat {}, given back",
        held.key, held.agent, held.chat_id
      ),
      Err(why) => eprintln!("double's slot {}: not given back, and not kept: {why}", held.key),
    }
  }
}

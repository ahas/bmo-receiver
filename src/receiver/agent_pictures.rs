//! The picture set on an agent from the console (docs/pictures.md, 「Profile pictures」): kept under the agent's
//! registration name whether or not it stands under any number yet, and worn under every number it stands under,
//! each such number holding a console-set row that names the agent's file. A number the agent takes later wears it
//! the moment the receiver first files the agent's words under it or gives the agent a number in a room.

use std::path::{Path, PathBuf};

use sea_orm::DatabaseConnection;

use super::{News, Receiver};
use crate::{Fault, all, now_ms, one, run};

/// The agent's picture as it is kept: its file and when it was set.
struct Kept {
  path: String,
  set_at: i64,
}

// The folder the console-set pictures of one kind are filed in.
fn set_folder(receiver: &Receiver, kind: &str) -> PathBuf {
  receiver.data_root().avatars().join("set").join(kind)
}

// The agent's file, named by its name's bytes in hexadecimal, so any registration name makes one plain file name.
fn agent_file(receiver: &Receiver, agent: &str) -> PathBuf {
  let named: String = agent.bytes().map(|byte| format!("{byte:02x}")).collect();

  set_folder(receiver, "agent").join(format!("{named}.jpg"))
}

async fn kept(db: &DatabaseConnection, agent: &str) -> Result<Option<Kept>, Fault> {
  one(
    db,
    r#"SELECT "path", "set_at" FROM "agent_avatar_sets" WHERE "agent_name" = $1"#,
    vec![agent.into()],
  )
  .await?
  .map(|row| -> Result<Kept, sea_orm::DbErr> {
    Ok(Kept {
      path: row.try_get("", "path")?,
      set_at: row.try_get("", "set_at")?,
    })
  })
  .transpose()
  .map_err(Fault::from)
}

/// Whether the file is the one an agent's picture holds, which no number's picture let go removes.
pub async fn held_by_agent(db: &DatabaseConnection, path: &str) -> Result<bool, Fault> {
  Ok(
    one(db, r#"SELECT 1 AS "one" FROM "agent_avatar_sets" WHERE "path" = $1"#, vec![path.into()])
      .await?
      .is_some(),
  )
}

// A number's console-set file let go where it was its own: a file an agent's picture holds stays.
async fn remove_own(db: &DatabaseConnection, path: &str) -> Result<(), Fault> {
  if !held_by_agent(db, path).await? {
    let _ = tokio::fs::remove_file(path).await;
  }

  Ok(())
}

/// Every number an agent stands under in the rooms: each a road or the record gave it, and each its own words are
/// archived under.
pub async fn numbers_of(db: &DatabaseConnection, agent: &str) -> Result<Vec<i64>, Fault> {
  let rows = all(
    db,
    r#"SELECT "sender_id" FROM "provider_users" WHERE "user_id" = $1
       UNION SELECT DISTINCT "sender_id" FROM "messages" WHERE "agent_name" = $1 AND "sender_id" IS NOT NULL
       ORDER BY 1"#,
    vec![agent.into()],
  )
  .await?;

  Ok(rows.iter().filter_map(|row| row.try_get::<i64>("", "sender_id").ok()).collect())
}

// The numbers wearing the agent's file now, beside every number the agent stands under.
async fn worn_or_stood(db: &DatabaseConnection, agent: &str, path: &str) -> Result<Vec<i64>, Fault> {
  let worn = all(
    db,
    r#"SELECT "subject_id" FROM "avatar_sets" WHERE "kind" = 'person' AND "path" = $1"#,
    vec![path.into()],
  )
  .await?;
  let mut numbers = numbers_of(db, agent).await?;

  numbers.extend(worn.iter().filter_map(|row| row.try_get::<i64>("", "subject_id").ok()));
  numbers.sort_unstable();
  numbers.dedup();

  Ok(numbers)
}

// The number made to wear the file as set at `set_at`, the file it wore before let go where it was its own.
async fn wear(db: &DatabaseConnection, number: i64, path: &str, set_at: i64) -> Result<(), Fault> {
  let before = one(
    db,
    r#"SELECT "path" FROM "avatar_sets" WHERE "kind" = 'person' AND "subject_id" = $1"#,
    vec![number.into()],
  )
  .await?
  .and_then(|row| row.try_get::<String>("", "path").ok());

  run(
    db,
    r#"INSERT INTO "avatar_sets" ("kind", "subject_id", "path", "set_at") VALUES ('person', $1, $2, $3)
       ON CONFLICT ("kind", "subject_id") DO UPDATE SET "path" = excluded."path", "set_at" = excluded."set_at""#,
    vec![number.into(), path.into(), set_at.into()],
  )
  .await?;

  match before {
    Some(before) if before != path => remove_own(db, &before).await,
    _ => Ok(()),
  }
}

/// The picture set on the agent: kept under its name, and worn anew under every number that wears it or that the
/// agent stands under. When it was set, and those numbers.
pub async fn set(receiver: &Receiver, agent: &str, jpeg: &[u8]) -> Result<(i64, Vec<i64>), Fault> {
  let db = receiver.db();
  let path = agent_file(receiver, agent);
  let folder = set_folder(receiver, "agent");
  let writing = path.with_extension("jpg.part");
  let failed = |why: std::io::Error| Fault::new(why.to_string());
  let path = path.to_string_lossy().into_owned();
  let set = now_ms();

  // The file is written beside its place and moved there, so a picture served while one is set is never half
  // written.
  tokio::fs::create_dir_all(&folder).await.map_err(failed)?;
  tokio::fs::write(&writing, jpeg).await.map_err(failed)?;
  tokio::fs::rename(&writing, &path).await.map_err(failed)?;
  run(
    db,
    r#"INSERT INTO "agent_avatar_sets" ("agent_name", "path", "set_at") VALUES ($1, $2, $3)
       ON CONFLICT ("agent_name") DO UPDATE SET "path" = excluded."path", "set_at" = excluded."set_at""#,
    vec![agent.into(), path.clone().into(), set.into()],
  )
  .await?;

  let numbers = worn_or_stood(db, agent, &path).await?;

  for number in &numbers {
    wear(db, *number, &path, set).await?;
  }

  Ok((set, numbers))
}

/// The picture set on the agent let go under every number that wore it or that the agent stands under, and under
/// its name: the numbers whose console-set picture went.
pub async fn unset(receiver: &Receiver, agent: &str) -> Result<Vec<i64>, Fault> {
  let db = receiver.db();
  let path = match kept(db, agent).await? {
    Some(picture) => picture.path,
    None => agent_file(receiver, agent).to_string_lossy().into_owned(),
  };
  let mut gone = vec![];

  for number in worn_or_stood(db, agent, &path).await? {
    let row = one(
      db,
      r#"DELETE FROM "avatar_sets" WHERE "kind" = 'person' AND "subject_id" = $1 RETURNING "path""#,
      vec![number.into()],
    )
    .await?;

    if let Some(worn) = row.and_then(|row| row.try_get::<String>("", "path").ok()) {
      if worn != path {
        remove_own(db, &worn).await?;
      }
      gone.push(number);
    }
  }

  let dropped = one(
    db,
    r#"DELETE FROM "agent_avatar_sets" WHERE "agent_name" = $1 RETURNING "path""#,
    vec![agent.into()],
  )
  .await?;

  if let Some(file) = dropped.and_then(|row| row.try_get::<String>("", "path").ok())
    && Path::new(&file).starts_with(receiver.data_root().avatars())
  {
    let _ = tokio::fs::remove_file(&file).await;
  }

  Ok(gone)
}

/// The agent's picture worn under numbers it has just taken, where one is set on it: a number already wearing it,
/// or one whose own picture was set from the console after the agent's, is left as it is. The console is told
/// where any number put it on. A failure is said in the journal and stops nothing the number was taken for.
pub async fn worn_by(db: &DatabaseConnection, news: &dyn News, agent: &str, numbers: &[i64]) {
  if let Err(why) = put_on(db, news, agent, numbers).await {
    eprintln!("bmo-receiver: the picture of agent {agent} was not put on its numbers {numbers:?}: {why}");
  }
}

async fn put_on(
  db: &DatabaseConnection,
  news: &dyn News,
  agent: &str,
  numbers: &[i64],
) -> Result<(), Fault> {
  let Some(picture) = kept(db, agent).await? else {
    return Ok(());
  };
  let mut put = false;

  for number in numbers {
    let worn = one(
      db,
      r#"SELECT "path", "set_at" FROM "avatar_sets" WHERE "kind" = 'person' AND "subject_id" = $1"#,
      vec![(*number).into()],
    )
    .await?;
    let stands = worn.is_some_and(|row| {
      row.try_get::<String>("", "path").is_ok_and(|path| path == picture.path)
        || row.try_get::<i64>("", "set_at").is_ok_and(|set_at| set_at >= picture.set_at)
    });

    if !stands {
      wear(db, *number, &picture.path, now_ms()).await?;
      put = true;
    }
  }

  if put {
    news.publish(&crate::events::avatars());
  }

  Ok(())
}

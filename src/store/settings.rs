//! The settings rows: one row a key, its value text (`docs/store.md`, 「The settings」). Each key is named by
//! the code that writes it; this module supplies only the reading and writing.

use sea_orm::ConnectionTrait;

use super::pool::{Fault, all, one, run};

/// The value a key holds, or none where no row holds it.
pub async fn get(db: &impl ConnectionTrait, key: &str) -> Result<Option<String>, Fault> {
  let row = one(db, r#"SELECT "value" FROM "settings" WHERE "key" = $1"#, vec![key.into()]).await?;

  Ok(row.map(|row| row.try_get::<String>("", "value")).transpose()?)
}

/// The key set to the value, whatever it held before.
pub async fn set(db: &impl ConnectionTrait, key: &str, value: &str) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "settings" ("key", "value") VALUES ($1, $2)
       ON CONFLICT ("key") DO UPDATE SET "value" = excluded."value""#,
    vec![key.into(), value.into()],
  )
  .await?;

  Ok(())
}

/// The key's row gone, where there was one.
pub async fn forget(db: &impl ConnectionTrait, key: &str) -> Result<(), Fault> {
  run(db, r#"DELETE FROM "settings" WHERE "key" = $1"#, vec![key.into()]).await?;

  Ok(())
}

/// The value a key held, taken so nothing reads it twice; none where no row held it.
pub async fn take(db: &impl ConnectionTrait, key: &str) -> Result<Option<String>, Fault> {
  let row =
    one(db, r#"DELETE FROM "settings" WHERE "key" = $1 RETURNING "value""#, vec![key.into()]).await?;

  Ok(row.map(|row| row.try_get::<String>("", "value")).transpose()?)
}

/// The numbers after a prefix on every key that carries it, in order: the members a family of keys is
/// kept for.
pub async fn numbered(db: &impl ConnectionTrait, prefix: &str) -> Result<Vec<i64>, Fault> {
  let rows = all(
    db,
    r#"SELECT "key" FROM "settings" WHERE "key" LIKE $1 ESCAPE '\'"#,
    vec![format!("{}%", prefix.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")).into()],
  )
  .await?;
  let mut numbers: Vec<i64> = rows
    .iter()
    .filter_map(|row| row.try_get::<String>("", "key").ok())
    .filter_map(|key| key.strip_prefix(prefix).and_then(|rest| rest.parse().ok()))
    .collect();

  numbers.sort_unstable();

  Ok(numbers)
}

/// Every key carrying a prefix, each with its value and with what follows the prefix: the members a family of
/// keys is kept for, named by what they carry beside the prefix.
pub async fn named(db: &impl ConnectionTrait, prefix: &str) -> Result<Vec<(String, String)>, Fault> {
  let rows = all(
    db,
    r#"SELECT "key", "value" FROM "settings" WHERE "key" LIKE $1 ESCAPE '\'"#,
    vec![format!("{}%", prefix.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")).into()],
  )
  .await?;
  let mut held: Vec<(String, String)> = vec![];

  for row in rows {
    let key: String = row.try_get("", "key")?;
    let value: String = row.try_get("", "value")?;
    let named = key.strip_prefix(prefix).unwrap_or_default().to_owned();

    held.push((named, value));
  }

  held.sort_unstable();

  Ok(held)
}

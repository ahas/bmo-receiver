//! The settings that name a room (docs/receiver.md, 「Removing a room's registration」): each kind of key is
//! declared beside the code that writes it, by its prefix and the shape of the parts after it, and the declaration
//! is the only way its keys are built. A removal takes every key a declaration matches; any other key holding the
//! room's number as one of its parts is listed and kept, since a private chat's number is its person's number too.
//! A setting whose value lists rooms is declared by its key, and a removal takes the room out of the list and
//! leaves the rest of it standing.

use sqlx::{AnyConnection, Row};

/// The part of a key that holds the room's number.
pub const ROOM: &str = "room";

/// One setting whose value is a JSON list of room numbers, in an order its owner keeps.
#[derive(Debug)]
pub struct RoomList {
  pub key: &'static str,
  pub why: &'static str,
}

/// One declared list of rooms.
pub struct Listed(pub &'static RoomList);

inventory::collect!(Listed);

/// Declares a setting listing rooms, beside the code that writes it.
#[macro_export]
macro_rules! room_list {
  ($list:expr) => {
    inventory::submit! { $crate::room_record::setting::Listed(&$list) }
  };
}

/// Every list of rooms declared in this binary.
pub fn listed() -> impl Iterator<Item = &'static RoomList> {
  inventory::iter::<Listed>.into_iter().map(|entry| entry.0)
}

impl RoomList {
  /// The rooms a held value lists, in its order; a value that is no list of numbers lists none.
  pub fn rooms(value: &str) -> Vec<i64> {
    serde_json::from_str::<Vec<i64>>(value).unwrap_or_default()
  }

  /// The value of a list of rooms.
  pub fn value(rooms: &[i64]) -> String {
    serde_json::Value::from(rooms).to_string()
  }

  // The value the setting holds in a database, where it holds one.
  async fn held(&self, database: &mut AnyConnection) -> Result<Option<String>, sqlx::Error> {
    sqlx::query(r#"SELECT "value" FROM settings WHERE "key" = $1"#)
      .bind(self.key)
      .fetch_optional(&mut *database)
      .await?
      .map(|row| row.try_get("value"))
      .transpose()
  }
}

/// One kind of setting naming a room: its prefix, ending in a colon, and the names of the colon-separated parts
/// after it, one of them `room`.
#[derive(Debug)]
pub struct RoomSetting {
  pub prefix: &'static str,
  pub parts: &'static [&'static str],
  pub why: &'static str,
}

/// One declared kind of setting.
pub struct Declared(pub &'static RoomSetting);

inventory::collect!(Declared);

/// Declares a kind of setting naming a room, beside the code that writes it.
#[macro_export]
macro_rules! room_setting {
  ($setting:expr) => {
    inventory::submit! { $crate::room_record::setting::Declared(&$setting) }
  };
}

/// Every kind of setting declared in this binary.
pub fn declared() -> impl Iterator<Item = &'static RoomSetting> {
  inventory::iter::<Declared>.into_iter().map(|entry| entry.0)
}

impl RoomSetting {
  // The parts in order, the room's put where its shape names it and the others in the order given.
  fn filled<'a>(&self, room: &'a str, others: &[&'a str]) -> Vec<&'a str> {
    assert_eq!(others.len() + 1, self.parts.len(), "{} takes {:?}", self.prefix, self.parts);

    let mut others = others.iter();

    self
      .parts
      .iter()
      .map(|part| if *part == ROOM { room } else { others.next().copied().unwrap_or_default() })
      .collect()
  }

  /// The key for a room, the other parts given in the order the shape names them.
  pub fn key(&self, room: i64, others: &[&str]) -> String {
    let room = room.to_string();

    format!("{}{}", self.prefix, self.filled(&room, others).join(":"))
  }

  /// The key as a SQL expression, each part given as an expression in the order the shape names them, the room's
  /// among them.
  pub fn sql(&self, parts: &[&str]) -> String {
    assert_eq!(parts.len(), self.parts.len(), "{} takes {:?}", self.prefix, self.parts);

    let mut said = format!("'{}'", self.prefix);

    for (at, part) in parts.iter().enumerate() {
      if at > 0 {
        said.push_str(" || ':'");
      }

      said.push_str(" || ");
      said.push_str(part);
    }

    said
  }

  /// A LIKE pattern for the keys whose parts are those given, `None` standing for any; the room's part is given
  /// among them in the order the shape names them.
  pub fn like(&self, parts: &[Option<&str>]) -> String {
    assert_eq!(parts.len(), self.parts.len(), "{} takes {:?}", self.prefix, self.parts);

    let parts: Vec<&str> = parts.iter().map(|part| part.unwrap_or("%")).collect();

    format!("{}{}", self.prefix, parts.join(":"))
  }

  /// Whether a key is of this kind and names the room.
  pub fn matches(&self, key: &str, room: i64) -> bool {
    let Some(rest) = key.strip_prefix(self.prefix) else { return false };
    let parts: Vec<&str> = rest.split(':').collect();
    let room = room.to_string();

    parts.len() == self.parts.len()
      && self.parts.iter().zip(&parts).all(|(name, part)| *name != ROOM || *part == room)
  }
}

/// The settings naming a room: those a declaration matches, which go, the undeclared keys holding its number as
/// one of their parts, which stay, and the declared lists holding it, which the room is taken out of.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settled {
  pub taken: Vec<String>,
  pub undeclared: Vec<String>,
  pub trimmed: Vec<&'static str>,
}

/// Reads the settings naming a room.
pub async fn settled(database: &mut AnyConnection, room: i64) -> Result<Settled, sqlx::Error> {
  let rows = sqlx::query(r#"SELECT "key" FROM settings WHERE "key" LIKE $1 ORDER BY "key""#)
    .bind(format!("%{room}%"))
    .fetch_all(&mut *database)
    .await?;
  let mut settled = Settled::default();

  for row in rows {
    let key: String = row.try_get("key")?;

    if declared().any(|setting| setting.matches(&key, room)) {
      settled.taken.push(key);
    } else if key.split(':').skip(1).any(|part| part == room.to_string()) {
      settled.undeclared.push(key);
    }
  }

  for list in listed() {
    if let Some(value) = list.held(database).await?
      && RoomList::rooms(&value).contains(&room)
    {
      settled.trimmed.push(list.key);
    }
  }

  Ok(settled)
}

/// Deletes the settings a declaration matched, and takes the room out of every list holding it.
pub async fn take(database: &mut AnyConnection, settled: &Settled, room: i64) -> Result<(), sqlx::Error> {
  for key in &settled.taken {
    sqlx::query(r#"DELETE FROM settings WHERE "key" = $1"#)
      .bind(key)
      .execute(&mut *database)
      .await?;
  }

  for list in listed().filter(|list| settled.trimmed.contains(&list.key)) {
    let Some(value) = list.held(database).await? else { continue };
    let rest: Vec<i64> = RoomList::rooms(&value).into_iter().filter(|listed| *listed != room).collect();

    sqlx::query(r#"UPDATE settings SET "value" = $1 WHERE "key" = $2"#)
      .bind(RoomList::value(&rest))
      .bind(list.key)
      .execute(&mut *database)
      .await?;
  }

  Ok(())
}

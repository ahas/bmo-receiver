//! The rooms the console stands over the rest, in the owner's order (`docs/boundary-api.md`, 「The pinned order」):
//! one setting holding a list of room numbers, a number that keeps its room through a rename. A room's removal
//! takes its number out of the list, by the declaration beside the list.

use sea_orm::DatabaseConnection;

use crate::Fault;
use crate::one;
use crate::room_record::setting::RoomList;
use crate::store::settings;

/// The setting the order stands in (`docs/store.md`, 「The settings」).
pub static PINNED: RoomList = RoomList {
  key: "console_pinned_rooms",
  why: "the console's pinned order",
};

crate::room_list!(PINNED);

/// The rooms the owner pinned over the rest, in the owner's order; a number the record no longer has stands for
/// nothing.
pub async fn pinned(db: &DatabaseConnection) -> Result<Vec<i64>, Fault> {
  Ok(
    settings::get(db, PINNED.key)
      .await?
      .map(|held| RoomList::rooms(&held))
      .unwrap_or_default(),
  )
}

/// The order set to the rooms given, a room named twice standing once at its first place; answers it.
pub async fn pin(db: &DatabaseConnection, rooms: &[i64]) -> Result<Vec<i64>, Fault> {
  let mut order: Vec<i64> = vec![];

  for room in rooms {
    if !order.contains(room) {
      order.push(*room);
    }
  }

  settings::set(db, PINNED.key, &RoomList::value(&order)).await?;

  Ok(order)
}

/// A room stood last in the order, where it does not stand already; answers the order.
pub async fn add(db: &DatabaseConnection, room: i64) -> Result<Vec<i64>, Fault> {
  let order = pinned(db).await?;

  pin(db, &[order, vec![room]].concat()).await
}

/// A room taken out of the order, where it stands in it; answers the order.
pub async fn remove(db: &DatabaseConnection, room: i64) -> Result<Vec<i64>, Fault> {
  let order: Vec<i64> = pinned(db).await?.into_iter().filter(|pinned| *pinned != room).collect();

  pin(db, &order).await
}

/// Whether the record holds a room: a title of its own, or a message said in it.
pub async fn held(db: &DatabaseConnection, room: i64) -> Result<bool, Fault> {
  let row = one(
    db,
    r#"SELECT 1 AS "one" WHERE EXISTS (SELECT 1 FROM "chat_titles" WHERE "chat_id" = $1)
       OR EXISTS (SELECT 1 FROM "messages" WHERE "chat_id" = $1)"#,
    vec![room.into()],
  )
  .await?;

  Ok(row.is_some())
}

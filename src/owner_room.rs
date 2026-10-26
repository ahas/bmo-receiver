//! The owner's private room stood up once on the record alone (docs/boundary-api.md, 「The service and its
//! commands」, `owner-room`; docs/receiver.md, 「Identity, and reaching the owner」): numbered as a console room is,
//! the record its one provider, the chat agent answering it under a number of the console's own, and the owner
//! there as the sender he is on every road.

use receiver_policy::roads::ROADS;
use sea_orm::DatabaseConnection;

use crate::roads::record::{next_room, next_sender};
use crate::run;
use crate::store::{Fault, settings};

/// The settings key the room stood up is kept under, so a second run finds it.
const MADE: &str = "owner_room_made";

/// The owner's private room: the one stood up before, else one stood up now for the chat agent under `title`;
/// answered with its number and whether it was stood up now.
pub async fn stand_up(db: &DatabaseConnection, agent: &str, title: &str) -> Result<(i64, bool), Fault> {
  if let Some(made) = settings::get(db, MADE).await?.and_then(|held| held.trim().parse::<i64>().ok()) {
    return Ok((made, false));
  }

  let road = ROADS.record.key.clone();
  let chat_id = next_room(db).await?;
  let place = format!("owner-room:{chat_id}");

  run(
    db,
    r#"INSERT INTO "provider_rooms" ("provider", "room_id", "chat_id", "title") VALUES ($1, $2, $3, $4)"#,
    vec![road.clone().into(), place.into(), chat_id.into(), title.into()],
  )
  .await?;
  crate::roads::providers::choose(db, chat_id, std::slice::from_ref(&road)).await?;
  run(
    db,
    r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, $2) ON CONFLICT ("chat_id") DO UPDATE SET "title" = excluded."title""#,
    vec![chat_id.into(), title.into()],
  )
  .await?;
  run(
    db,
    r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ($1, $2)"#,
    vec![chat_id.into(), agent.into()],
  )
  .await?;

  // The chat agent's number on the record, one of the console's own, where it holds none there yet.
  let numbered = crate::one(
    db,
    r#"SELECT "sender_id" FROM "provider_users" WHERE "provider" = $1 AND "user_id" = $2"#,
    vec![road.clone().into(), agent.into()],
  )
  .await?;

  if numbered.is_none() {
    let sender = next_sender(db, &road).await?;

    run(
      db,
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ($1, $2, $3)"#,
      vec![road.into(), agent.into(), sender.into()],
    )
    .await?;
  }

  settings::set(db, MADE, &chat_id.to_string()).await?;

  Ok((chat_id, true))
}

//! A room's own word from the shell, written where the receiver reads it: a member muted, an operating
//! notice switched, and whether BMO opens the pictures it stores (docs/pictures.md, 「Whether BMO looks」;
//! docs/voiced-notices.md).

use sea_orm::DatabaseConnection;

use crate::receiver::rows::{set_muted, set_photo_look};
use crate::store::settings;
use crate::verbs::VerbError;
use crate::{Fault, all, one, run};

/// The setting every room without a word of its own follows: BMO looks where it holds `on`.
const PHOTOS_LOOK: &str = "photos_look";

/// A member muted in a room, or let speak again. The owner is never muted, so muting him is refused
/// where his number is known and refused too where it is not, since he could not be told apart.
pub async fn mute(
  db: &DatabaseConnection,
  chat_id: i64,
  user_id: i64,
  on: bool,
  owner_id: Option<i64>,
) -> Result<Vec<String>, VerbError> {
  if on {
    match owner_id {
      None => {
        return Err(VerbError::Refused(
          "the owner is not known, so nobody is muted: the owner is not configured".into(),
        ));
      }
      Some(owner) if owner == user_id => {
        return Err(VerbError::Refused("the owner cannot be muted".into()));
      }
      Some(_) => {}
    }
  }

  set_muted(db, chat_id, user_id, on).await?;

  Ok(vec![if on { "muted" } else { "unmuted" }.to_owned()])
}

/// What `photos look` asks: looking, not looking, or a room's own word taken off again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
  On,
  Off,
  Default,
}

/// What each room does with the pictures it stores, under the default: every room the archive holds
/// messages of, so a room that has said nothing is still shown doing what the default has it doing.
pub async fn photos_status(db: &DatabaseConnection) -> Result<Vec<String>, VerbError> {
  let default = looks_by_default(db).await?;
  let mut lines = vec![
    if default {
      "default: BMO looks at photos"
    } else {
      "default: BMO does not look at photos"
    }
    .to_owned(),
  ];
  let rooms = all(
    db,
    r#"SELECT DISTINCT m."chat_id", p."look", t."title"
       FROM "messages" m
       LEFT JOIN "photo_chats" p ON p."chat_id" = m."chat_id"
       LEFT JOIN "chat_titles" t ON t."chat_id" = m."chat_id"
       ORDER BY m."chat_id""#,
    vec![],
  )
  .await?;

  for room in rooms {
    let chat_id = room.try_get::<i64>("", "chat_id").map_err(Fault::from)?;
    let own = room.try_get::<Option<i64>>("", "look").map_err(Fault::from)?;
    let title = room
      .try_get::<Option<String>>("", "title")
      .map_err(Fault::from)?
      .unwrap_or_else(|| chat_id.to_string());
    let looks = own.map_or(default, |look| look == 1);

    lines.push(format!(
      "{} — {title} ({chat_id}){}",
      if looks { "looks " } else { "does not look" },
      if own.is_none() { ", follows the default" } else { ", its own word" },
    ));
  }

  Ok(lines)
}

/// The default every room without a word of its own follows, or one room's own word taken off again.
pub async fn photos_look(
  db: &DatabaseConnection,
  look: Look,
  chat_id: Option<i64>,
) -> Result<Vec<String>, VerbError> {
  let Some(chat_id) = chat_id else {
    if look == Look::Default {
      return Err(VerbError::Refused(
        "photos: 'look default' is a room's word to take off, so it needs --chat <id>".into(),
      ));
    }

    // A room that has said something keeps saying it.
    if look == Look::On {
      settings::set(db, PHOTOS_LOOK, "on").await?;
    } else {
      settings::forget(db, PHOTOS_LOOK).await?;
    }

    let mut lines = vec![format!(
      "photos: the default is now {}",
      if look == Look::On { "looking" } else { "not looking" }
    )];

    lines.extend(photos_status(db).await?);

    return Ok(lines);
  };
  let name = room_name(db, chat_id).await?;

  match look {
    Look::Default => {
      run(db, r#"DELETE FROM "photo_chats" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?;

      let looks = looks_by_default(db).await?;

      Ok(vec![format!(
        "photos: {name} follows the default again — {}",
        if looks { "looks" } else { "does not look" },
      )])
    }
    Look::On | Look::Off => {
      set_photo_look(db, chat_id, look == Look::On).await?;

      Ok(vec![format!(
        "photos: {name} {}",
        if look == Look::On { "looks at them" } else { "does not look at them" },
      )])
    }
  }
}

async fn looks_by_default(db: &DatabaseConnection) -> Result<bool, VerbError> {
  Ok(settings::get(db, PHOTOS_LOOK).await?.as_deref() == Some("on"))
}

// A room by the title the archive keeps for it, or by its number where it keeps none.
async fn room_name(db: &DatabaseConnection, chat_id: i64) -> Result<String, VerbError> {
  let row =
    one(db, r#"SELECT "title" FROM "chat_titles" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?;

  Ok(match row {
    Some(row) => row.try_get::<String>("", "title").map_err(Fault::from)?,
    None => chat_id.to_string(),
  })
}

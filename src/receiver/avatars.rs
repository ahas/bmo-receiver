//! The profile pictures each chat service keeps, taken into the receiver so the console reads every one of them
//! from the receiver alone (docs/pictures.md, 「Profile pictures」): each subject a road's word names asked of that
//! road, each road's list walked as the service starts, and then every room and person whose words came on a road
//! and that holds no picture asked of that road by its subject. A picture is kept under the number the road gave
//! its subject, laid on white as a JPEG under the data root's avatars folder.

use std::path::Path;

use bmo_chat_client::{AvatarsQuery, ChatAvatarKind};
use receiver_policy::roads::ROADS;
use receiver_ports::chat::Chat;

use super::{Receiver, pictures};
use crate::{Fault, all, now_ms, one, run};

/// What became of one subject's picture asked of a road.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taken {
  /// The road served a picture, and it is kept.
  Kept,
  /// The road keeps none, and the one kept is let go.
  LetGo,
  /// Nothing kept changed: the road did not answer, served no picture that could be read, or keeps none where
  /// none was kept either.
  Unchanged,
}

/// A picture laid on white and drawn as a JPEG at the quality the pictures are kept at; none where it cannot be
/// read as a picture.
pub async fn drawn(bytes: Vec<u8>) -> Option<Vec<u8>> {
  let quality = ROADS.pictures.kept_quality;

  tokio::task::spawn_blocking(move || {
    pictures::decode(&bytes).and_then(|picture| pictures::jpeg(&pictures::on_white(&picture), quality))
  })
  .await
  .ok()
  .and_then(Result::ok)
}

// The picture filed as the subject's under the data root's avatars folder, its row naming when it was taken. The
// file is written beside its place and moved there, so a picture served while one is taken is never half written.
async fn keep(receiver: &Receiver, kind: &str, id: i64, jpeg: &[u8], taken_at: i64) -> Result<(), Fault> {
  let folder = receiver.data_root().avatars().join(kind);
  let path = folder.join(format!("{id}.jpg"));
  let writing = folder.join(format!(".{id}.jpg.part"));
  let failed = |why: std::io::Error| Fault::new(why.to_string());

  tokio::fs::create_dir_all(&folder).await.map_err(failed)?;
  tokio::fs::write(&writing, jpeg).await.map_err(failed)?;
  tokio::fs::rename(&writing, &path).await.map_err(failed)?;
  run(
    receiver.db(),
    r#"INSERT INTO "avatars" ("kind", "subject_id", "photo_id", "path", "updated_at") VALUES ($1, $2, NULL, $3, $4)
       ON CONFLICT ("kind", "subject_id") DO UPDATE
         SET "photo_id" = NULL, "path" = excluded."path", "updated_at" = excluded."updated_at""#,
    vec![kind.into(), id.into(), path.to_string_lossy().into_owned().into(), taken_at.into()],
  )
  .await
  .map(|_| ())
}

// The subject's kept picture let go, its file with it where the file stands in the avatars folder: whether one was
// kept.
async fn let_go(receiver: &Receiver, kind: &str, id: i64) -> Result<bool, Fault> {
  let gone = one(
    receiver.db(),
    r#"DELETE FROM "avatars" WHERE "kind" = $1 AND "subject_id" = $2 RETURNING "path""#,
    vec![kind.into(), id.into()],
  )
  .await?;
  let Some(row) = gone else { return Ok(false) };

  if let Ok(Some(path)) = row.try_get::<Option<String>>("", "path")
    && Path::new(&path).starts_with(receiver.data_root().avatars())
  {
    let _ = tokio::fs::remove_file(&path).await;
  }

  Ok(true)
}

// When the picture kept for the subject was taken, where one is kept with a file.
async fn kept_at(receiver: &Receiver, kind: &str, id: i64) -> Result<Option<i64>, Fault> {
  one(
    receiver.db(),
    r#"SELECT "updated_at" FROM "avatars"
       WHERE "kind" = $1 AND "subject_id" = $2 AND "path" IS NOT NULL AND "path" <> ''"#,
    vec![kind.into(), id.into()],
  )
  .await?
  .map(|row| row.try_get::<i64>("", "updated_at"))
  .transpose()
  .map_err(Fault::from)
}

/// One subject's picture asked of a road and kept as the road answers: served, it is kept as taken at `taken_at`,
/// else at this moment; answered `not-found`, the one kept is let go; not answered, the one kept stands.
pub async fn take(
  receiver: &Receiver,
  key: &str,
  road: &dyn Chat,
  kind: ChatAvatarKind,
  id: i64,
  taken_at: Option<i64>,
) -> Taken {
  let named = kind.to_string();
  let answered = match road.avatar(kind, id).await {
    Ok(answered) => answered,
    Err(why) => {
      eprintln!("bmo-receiver: the {key} road did not answer the picture of {named} {id}: {why}");

      return Taken::Unchanged;
    }
  };
  let done = match answered {
    Some(picture) => {
      let Some(jpeg) = drawn(picture.bytes).await else {
        eprintln!("bmo-receiver: the {key} road's picture of {named} {id} could not be read as a picture");

        return Taken::Unchanged;
      };

      keep(receiver, &named, id, &jpeg, taken_at.unwrap_or_else(now_ms))
        .await
        .map(|()| Taken::Kept)
    }
    None => let_go(receiver, &named, id)
      .await
      .map(|gone| if gone { Taken::LetGo } else { Taken::Unchanged }),
  };

  done.unwrap_or_else(|why| {
    eprintln!("bmo-receiver: the {key} road's picture of {named} {id} was not kept: {why}");

    Taken::Unchanged
  })
}

/// The subjects a road's word names, each asked of that road; a road the configuration does not list is said in
/// the log and asked nothing. The console is told once the pictures are taken.
pub async fn changed(receiver: &Receiver, key: &str, rooms: &[i64], people: &[i64]) {
  if let Some(road) = receiver.roads().chats().road(key).cloned() {
    let subjects = rooms
      .iter()
      .map(|id| (ChatAvatarKind::Room, *id))
      .chain(people.iter().map(|id| (ChatAvatarKind::Person, *id)));

    for (kind, id) in subjects {
      take(receiver, key, road.as_ref(), kind, id, None).await;
    }
  } else {
    eprintln!(
      "bmo-receiver: word that pictures changed came from {key}, a road the configuration does not list"
    );
  }

  receiver.news().publish(&crate::events::avatars());
}

// Every picture on a road's list taken later than the one kept, the list walked a page at a time: how many were
// kept. A page the road does not answer ends the walk.
async fn walk_list(receiver: &Receiver, key: &str, road: &dyn Chat) -> usize {
  let mut after: Option<i64> = None;
  let mut kept = 0;

  loop {
    let page = match road.avatars(&AvatarsQuery { after, limit: None }).await {
      Ok(page) => page,
      Err(why) => {
        eprintln!("bmo-receiver: the {key} road did not answer its list of pictures: {why}");

        return kept;
      }
    };

    for listed in &page.items {
      let standing = kept_at(receiver, &listed.kind.to_string(), listed.id).await.unwrap_or(None);

      if standing.is_none_or(|standing| listed.taken_at > standing)
        && take(receiver, key, road, listed.kind, listed.id, Some(listed.taken_at)).await == Taken::Kept
      {
        kept += 1;
      }
    }

    match page.items.last() {
      Some(last) if page.more => after = Some(last.taken_at),
      _ => return kept,
    }
  }
}

// The rooms and people whose words came on a road and that hold no picture a chat service gave.
async fn unpictured(receiver: &Receiver, key: &str) -> Result<Vec<(ChatAvatarKind, i64)>, Fault> {
  let rows = all(
    receiver.db(),
    r#"SELECT 'room' AS "kind", "chat_id" AS "subject_id" FROM "messages" WHERE "via" = $1
       UNION SELECT 'person', "sender_id" FROM "messages" WHERE "via" = $1 AND "sender_id" IS NOT NULL
       EXCEPT SELECT "kind", "subject_id" FROM "avatars" WHERE "path" IS NOT NULL AND "path" <> ''
       ORDER BY 1, 2"#,
    vec![key.into()],
  )
  .await?;

  Ok(
    rows
      .iter()
      .filter_map(|row| {
        let kind = match row.try_get::<String>("", "kind").ok()?.as_str() {
          "room" => ChatAvatarKind::Room,
          _ => ChatAvatarKind::Person,
        };

        Some((kind, row.try_get::<i64>("", "subject_id").ok()?))
      })
      .collect(),
  )
}

/// One road's pictures taken as the service starts: its list walked, then every room and person whose words came
/// on it and that holds no picture asked of it by its subject. How many pictures were kept.
pub async fn walk(receiver: &Receiver, key: &str) -> usize {
  let Some(road) = receiver.roads().chats().road(key).cloned() else { return 0 };
  let mut kept = walk_list(receiver, key, road.as_ref()).await;
  let asked = unpictured(receiver, key).await.unwrap_or_else(|why| {
    eprintln!("bmo-receiver: the subjects with no picture on the {key} road were not read: {why}");

    vec![]
  });

  for (kind, id) in asked {
    if take(receiver, key, road.as_ref(), kind, id, None).await == Taken::Kept {
      kept += 1;
    }
  }

  kept
}

/// Every road's pictures taken, one road after another, the console told once where any was kept.
pub async fn walk_every(receiver: &Receiver) {
  let keys: Vec<String> = receiver.roads().chats().names().map(str::to_owned).collect();
  let mut kept = 0;

  for key in keys {
    kept += walk(receiver, &key).await;
  }

  if kept > 0 {
    receiver.news().publish(&crate::events::avatars());
  }
}

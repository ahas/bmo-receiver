//! A profile picture set from the console (docs/pictures.md, 「Profile pictures」): put on a room, a person, or an
//! agent under every number it stands under in the rooms, kept beside the picture a chat service hands over, the
//! newer of the two standing; let go, the chat service's stands again.

use axum::extract::{Multipart, Path, State};
use receiver_codes::AvatarRefusal;
use receiver_policy::files::FILES;
use receiver_policy::roads::ROADS;
use serde_json::{Value, json};

use super::{Answered, Refusal, Served, answered, code, store};
use crate::receiver::{Receiver, pictures};
use crate::store::Fault;
use crate::{all, now_ms, one, run};

// The one `picture` of the form, as its bytes.
async fn picture(mut form: Multipart) -> Result<Vec<u8>, Refusal> {
  while let Ok(Some(field)) = form.next_field().await {
    if field.name() == Some("picture") {
      let bytes = field.bytes().await.map_err(|_| code(&AvatarRefusal::NotAPicture))?.to_vec();
      let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);

      if size > FILES.largest_bytes {
        return Err(code(&AvatarRefusal::PictureTooLarge {
          size_bytes: size,
          largest_bytes: FILES.largest_bytes,
        }));
      }

      return Ok(bytes);
    }
  }

  Err(code(&AvatarRefusal::NoPictureGiven))
}

// The picture laid on white and drawn as a JPEG.
async fn drawn(bytes: Vec<u8>) -> Result<Vec<u8>, Refusal> {
  let quality = ROADS.pictures.kept_quality;

  tokio::task::spawn_blocking(move || {
    pictures::decode(&bytes).and_then(|picture| pictures::jpeg(&pictures::on_white(&picture), quality))
  })
  .await
  .ok()
  .and_then(Result::ok)
  .ok_or_else(|| code(&AvatarRefusal::NotAPicture))
}

// Every number an agent stands under in the rooms: each a road or the record gave it, and each its own words are
// archived under.
async fn numbers_of(receiver: &Receiver, agent: &str) -> Result<Vec<i64>, Fault> {
  let rows = all(
    receiver.db(),
    r#"SELECT "sender_id" FROM "provider_users" WHERE "user_id" = $1
       UNION SELECT DISTINCT "sender_id" FROM "messages" WHERE "agent_name" = $1 AND "sender_id" IS NOT NULL
       ORDER BY 1"#,
    vec![agent.into()],
  )
  .await?;

  Ok(rows.iter().filter_map(|row| row.try_get::<i64>("", "sender_id").ok()).collect())
}

// The rooms or people a subject names: a room's or a person's number, or every number an agent stands under.
async fn subjects(
  receiver: &Receiver,
  kind: &str,
  subject: &str,
) -> Result<(&'static str, Vec<i64>), Refusal> {
  let number = || {
    subject.trim().parse::<i64>().map_err(|_| {
      code(&AvatarRefusal::UnknownKind {
        kind: kind.to_owned(),
      })
    })
  };

  match kind {
    "room" => Ok(("room", vec![number()?])),
    "person" => Ok(("person", vec![number()?])),
    "agent" => {
      let numbers = numbers_of(receiver, subject).await.map_err(store)?;

      if numbers.is_empty() {
        return Err(code(&AvatarRefusal::NoNumber {
          agent: subject.to_owned(),
        }));
      }

      Ok(("person", numbers))
    }
    _ => Err(code(&AvatarRefusal::UnknownKind {
      kind: kind.to_owned(),
    })),
  }
}

// The picture set on one room or person, filed under the data root's avatars/set folder: when it was set.
async fn keep(receiver: &Receiver, kind: &str, subject: i64, jpeg: &[u8]) -> Result<i64, Fault> {
  let folder = receiver.data_root().avatars().join("set").join(kind);
  let path = folder.join(format!("{subject}.jpg"));
  let set = now_ms();

  tokio::fs::create_dir_all(&folder).await.map_err(|why| Fault::new(why.to_string()))?;
  tokio::fs::write(&path, jpeg).await.map_err(|why| Fault::new(why.to_string()))?;
  run(
    receiver.db(),
    r#"INSERT INTO "avatar_sets" ("kind", "subject_id", "path", "set_at") VALUES ($1, $2, $3, $4)
       ON CONFLICT ("kind", "subject_id") DO UPDATE SET "path" = excluded."path", "set_at" = excluded."set_at""#,
    vec![kind.into(), subject.into(), path.to_string_lossy().into_owned().into(), set.into()],
  )
  .await?;

  Ok(set)
}

/// A picture set on a room, a person or an agent.
pub async fn set(
  State(served): State<Served>,
  Path((kind, subject)): Path<(String, String)>,
  form: Multipart,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let (kept_as, numbers) = subjects(&receiver, &kind, &subject).await?;
  let jpeg = drawn(picture(form).await?).await?;
  let mut items: Vec<Value> = vec![];

  for number in numbers {
    let set = keep(&receiver, kept_as, number, &jpeg).await.map_err(store)?;

    items.push(json!({ "kind": kept_as, "id": number, "takenAt": set }));
  }

  receiver.news().publish(&crate::events::avatars());
  answered(json!({ "items": items }))
}

/// The picture set on a room, a person or an agent let go, so the one its chat service handed over stands.
pub async fn unset(
  State(served): State<Served>,
  Path((kind, subject)): Path<(String, String)>,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let (kept_as, numbers) = subjects(&receiver, &kind, &subject).await?;
  let mut items: Vec<Value> = vec![];

  for number in numbers {
    let gone = one(
      receiver.db(),
      r#"DELETE FROM "avatar_sets" WHERE "kind" = $1 AND "subject_id" = $2 RETURNING "path""#,
      vec![kept_as.into(), number.into()],
    )
    .await
    .map_err(store)?;

    if let Some(path) = gone.and_then(|row| row.try_get::<String>("", "path").ok()) {
      let _ = tokio::fs::remove_file(&path).await;

      items.push(json!({ "kind": kept_as, "id": number }));
    }
  }

  receiver.news().publish(&crate::events::avatars());
  answered(json!({ "items": items }))
}

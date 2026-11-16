//! A profile picture set from the console (docs/pictures.md, 「Profile pictures」): put on a room, a person, or an
//! agent, the agent's kept under its name and worn under every number it stands under in the rooms, kept beside the
//! picture a chat service hands over, the newer of the two standing; let go, the chat service's stands again.

use axum::extract::{Multipart, Path, State};
use axum::http::HeaderMap;
use receiver_codes::AvatarRefusal;
use receiver_policy::files::FILES;
use serde_json::{Value, json};

use super::writable::Writer;
use super::{Answered, Refusal, Served, answered, code, store};
use crate::receiver::{Receiver, agent_pictures, avatars};
use crate::store::Fault;
use crate::{now_ms, one, run};

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
  avatars::drawn(bytes).await.ok_or_else(|| code(&AvatarRefusal::NotAPicture))
}

// The room or person a subject names by its number; an agent is kept under its name instead.
fn subjects(kind: &str, subject: &str) -> Result<(&'static str, Vec<i64>), Refusal> {
  let unknown = || {
    code(&AvatarRefusal::UnknownKind {
      kind: kind.to_owned(),
    })
  };
  let number = || subject.trim().parse::<i64>().map_err(|_| unknown());

  match kind {
    "room" => Ok(("room", vec![number()?])),
    "person" => Ok(("person", vec![number()?])),
    _ => Err(unknown()),
  }
}

// The agent a subject names, by its registration name, refused as an unknown kind where it names none.
fn agent_named(subject: &str) -> Result<&str, Refusal> {
  let agent = subject.trim();

  if agent.is_empty() {
    return Err(code(&AvatarRefusal::UnknownKind {
      kind: "agent".to_owned(),
    }));
  }

  Ok(agent)
}

// Whether the console relay's caller may set the subject's picture: an agent's and a room's the administrator's
// alone, a person's the administrator's and that of any user whose own senders hold the person.
async fn allowed(
  receiver: &Receiver,
  asked: &HeaderMap,
  kind: &str,
  numbers: &[i64],
) -> Result<(), Refusal> {
  let header =
    |name: &str| asked.get(name).and_then(|value| value.to_str().ok()).map(str::trim).unwrap_or_default();
  let forbidden = || {
    code(&AvatarRefusal::PictureForbidden {
      kind: kind.to_owned(),
    })
  };

  if header("owner-role") == "administrator" {
    return Ok(());
  }

  let user = header("owner-key");

  if kind != "person" || user.is_empty() {
    return Err(forbidden());
  }

  let own = receiver.units().web.senders_of_user(user).await.unwrap_or_default();

  if numbers.iter().all(|number| own.contains(number)) {
    Ok(())
  } else {
    Err(forbidden())
  }
}

// A room's picture is a write in the room, refused in another account's private room.
async fn in_rooms(
  receiver: &Receiver,
  writer: &Writer,
  kept_as: &str,
  numbers: &[i64],
) -> Result<(), Refusal> {
  if kept_as != "room" {
    return Ok(());
  }

  for room in numbers {
    writer.room(receiver, *room).await?;
  }

  Ok(())
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
  asked: HeaderMap,
  writer: Writer,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  // A room's and an agent's picture are refused to anyone but the administrator before the subject is looked up.
  if kind != "person" {
    allowed(&receiver, &asked, &kind, &[]).await?;
  }

  if kind == "agent" {
    let agent = agent_named(&subject)?;
    let jpeg = drawn(picture(writer.form().await?).await?).await?;
    let (set, numbers) = agent_pictures::set(&receiver, agent, &jpeg).await.map_err(store)?;
    let items: Vec<Value> = numbers
      .iter()
      .map(|number| json!({ "kind": "person", "id": number, "takenAt": set }))
      .collect();

    receiver.news().publish(&crate::events::avatars());
    return answered(json!({ "items": items }));
  }

  let (kept_as, numbers) = subjects(&kind, &subject)?;

  allowed(&receiver, &asked, &kind, &numbers).await?;
  in_rooms(&receiver, &writer, kept_as, &numbers).await?;
  let jpeg = drawn(picture(writer.form().await?).await?).await?;
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
  asked: HeaderMap,
  writer: Writer,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  // A room's and an agent's picture are refused to anyone but the administrator before the subject is looked up.
  if kind != "person" {
    allowed(&receiver, &asked, &kind, &[]).await?;
  }

  if kind == "agent" {
    let gone = agent_pictures::unset(&receiver, agent_named(&subject)?).await.map_err(store)?;
    let items: Vec<Value> = gone.iter().map(|number| json!({ "kind": "person", "id": number })).collect();

    receiver.news().publish(&crate::events::avatars());
    return answered(json!({ "items": items }));
  }

  let (kept_as, numbers) = subjects(&kind, &subject)?;

  allowed(&receiver, &asked, &kind, &numbers).await?;
  in_rooms(&receiver, &writer, kept_as, &numbers).await?;
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
      // A file an agent's picture holds stays for the agent and the other numbers wearing it.
      if !agent_pictures::held_by_agent(receiver.db(), &path).await.map_err(store)? {
        let _ = tokio::fs::remove_file(&path).await;
      }

      items.push(json!({ "kind": kept_as, "id": number }));
    }
  }

  receiver.news().publish(&crate::events::avatars());
  answered(json!({ "items": items }))
}

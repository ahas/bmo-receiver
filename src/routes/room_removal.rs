//! A room's registration removed from the console (docs/receiver.md, 「Removing a room's registration」): what
//! the removal would take, answered with nothing written, and the removal itself, run to its end whatever
//! becomes of the request. Both are refused to anyone but the administrator. What a room holds, what stays, what
//! refuses and what warns is read off the room record's declarations, so nothing here names a table, a folder or
//! a holder of live state; every other unit's part goes through that unit.

use std::path::Path;

use axum::extract::{RawQuery, State};
use serde::Deserialize;
use serde_json::{Value, json};

use receiver_codes::{RemovalLeft, RoomRemovalRefusal};
use receiver_ports::http::PortFailure;

use sqlx::{AnyConnection, Connection};

use crate::receiver::Receiver;
use crate::room_record::forget::forget;
use crate::room_record::{
  Act, Counted, Facing, Held, count, files, folders, record, refusals, take, warnings,
};
use crate::routes::instructions::HOLDER_KEY;
use crate::routes::writable::WritableRoom;
use crate::routes::{Answered, Refusal, Served, answered, code, ready, store};
use crate::store::{Fault, settings};

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemovalQuery {
  #[serde(default)]
  by: String,

  /// What the removal would do, nothing written.
  #[serde(default)]
  dry_run: bool,
}

// A declared holder, as its declaration is kept: the base its unit is reached at.
#[derive(Deserialize)]
pub struct Declared {
  address: String,
}

async fn administrator(receiver: &Receiver, by: &str) -> Result<(), Refusal> {
  if super::is_administrator(receiver, by).await {
    Ok(())
  } else {
    Err(code(&RoomRemovalRefusal::NotAdministrator))
  }
}

// The database opened by place, as the configuration gives it: a room's record is read off the schema the
// database has now, the same way a copy of it is.
async fn open(receiver: &Receiver) -> Result<AnyConnection, Refusal> {
  let Some(place) = receiver.place() else { return Err(ready("the database")) };

  sqlx::any::install_default_drivers();

  AnyConnection::connect(&place).await.map_err(failed)
}

fn counted_view(counted: &Counted) -> Value {
  let mut view = json!({ "table": counted.table, "rows": counted.rows, "why": counted.why });

  if let Act::Clear(columns) = counted.act {
    view["cleared"] = json!(columns);
  }

  view
}

// What goes, and what stays with the columns naming the room cleared or kept as they are.
fn split(counted: &[Counted]) -> (Vec<Value>, Vec<Value>) {
  let goes = counted
    .iter()
    .filter(|counted| counted.act == Act::Take && counted.rows > 0)
    .map(counted_view);
  let stays = counted
    .iter()
    .filter(|counted| counted.act != Act::Take && counted.rows > 0)
    .map(counted_view);

  (goes.collect(), stays.collect())
}

fn folder_view(receiver: &Receiver, held: &Held) -> Value {
  let files = files_under(&held.path);
  let mut view = json!({ "what": held.what, "path": shown(receiver, &held.path), "files": files.len() });

  if held.named {
    view["documents"] = json!(files);
  }

  view
}

// Every file under a folder, never through a link, by its path relative to the folder.
fn files_under(root: &Path) -> Vec<String> {
  fn walk(root: &Path, at: &Path, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(at) else {
      return;
    };

    for entry in entries.flatten() {
      let Ok(kind) = entry.file_type() else {
        continue;
      };
      let path = entry.path();

      if kind.is_dir() {
        walk(root, &path, found);
      } else {
        found.push(path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned());
      }
    }
  }

  let mut found = vec![];

  walk(root, root, &mut found);
  found.sort();
  found
}

// A path as the answer names it: under the data root where it stands there.
fn shown(receiver: &Receiver, path: &Path) -> String {
  path
    .strip_prefix(receiver.data_root().path())
    .unwrap_or(path)
    .to_string_lossy()
    .into_owned()
}

// Every unit declared a holder of rows naming the room, asked what it holds of it behind the token of the
// configured unit at its address (docs/boundary-api.md, 「A declared holder」).
async fn holders(receiver: &Receiver, room: i64) -> Vec<Value> {
  let named = settings::named(receiver.db(), HOLDER_KEY).await.unwrap_or_default();
  let mut answers: Vec<Value> = vec![];

  for (unit, written) in named {
    let asked = match serde_json::from_str::<Declared>(&written) {
      Err(why) => Err(PortFailure::Undecodable {
        url: unit.clone(),
        why: why.to_string(),
      }),
      Ok(holder) => receiver.units().holders.holdings(&holder.address, room).await,
    };
    let (held, failure) = match asked {
      Ok(held) => (json!(held), Value::Null),
      Err(why) => (json!([]), json!(format!("{why}"))),
    };

    answers.push(json!({ "unit": unit, "holdings": held, "failure": failure }));
  }

  answers
}

/// What the removal would take and leave, every count read the way the removal takes it.
async fn shown_removal(receiver: &Receiver, room: i64) -> Result<Value, Refusal> {
  let facing = Facing { receiver, room };
  let said = refusals(&facing).await.unwrap_or_default();
  let refused = said.first();
  let warnings = warnings(&facing).await.unwrap_or_default();
  let folders = folders(&facing).await.unwrap_or_default();
  let mut archive = open(receiver).await?;
  let read = async {
    let record = record(&mut archive, room).await?;
    let counted = count(&mut archive, &record, room).await?;
    let pictures = files(&mut archive, &record, room).await?;

    Ok::<_, sqlx::Error>((record, counted, pictures))
  }
  .await;
  let _ = archive.close().await;
  let (record, counted, pictures) = read.map_err(failed)?;
  let (goes, stays) = split(&counted);
  let units = holders(receiver, room).await;
  let agency = receiver.units().agency.forget_room(room, true).await;

  Ok(json!({
    "room": room,
    "refused": refused.map(|why| code(why).code),
    "warned": warnings,
    "warns": units,
    "goes": goes,
    "stays": stays,
    "pictures": pictures,
    "settings": {
      "taken": record.settings.taken,
      "undeclared": record.settings.undeclared,
      "trimmed": record.settings.trimmed,
    },
    "folders": folders.iter().map(|held| folder_view(receiver, held)).collect::<Vec<_>>(),
    "units": json!({ "agency": unit_answer(agency) }),
  }))
}

// What a unit's part came to: what it answered, or why it did not.
fn unit_answer<T: serde::Serialize>(asked: Result<T, PortFailure>) -> Value {
  match asked {
    Ok(one) => json!(one),
    Err(why) => json!({ "failure": format!("{why}") }),
  }
}

/// The removal carried out: what it took, and anything it could not remove.
async fn carry(receiver: &Receiver, room: i64, by: &str) -> Result<Value, Refusal> {
  let facing = Facing { receiver, room };
  let folders = folders(&facing).await.unwrap_or_default();
  let mut archive = open(receiver).await?;
  let written = async {
    // Read before anything moves, so what is written is what the confirmation showed.
    let record = record(&mut archive, room).await?;
    let pictures = files(&mut archive, &record, room).await?;

    Ok::<_, sqlx::Error>((record, pictures))
  }
  .await;
  let _ = archive.close().await;
  let (record, pictures) = written.map_err(failed)?;

  // The other units' parts go through them, and every holder of live per-room state forgets the room.
  let agency = receiver.units().agency.forget_room(room, false).await;
  forget(receiver, room).await;

  let mut archive = open(receiver).await?;
  let taken = async { take(&mut archive, &record, room).await }.await;
  let _ = archive.close().await;
  let counted = taken.map_err(failed)?;

  let mut left: Vec<RemovalLeft> = vec![];
  let mut removed_pictures: Vec<String> = vec![];

  for path in pictures {
    let at = Path::new(&path);

    if !under(receiver, at) {
      left.push(RemovalLeft::OutsideDataRoot { path });
      continue;
    }

    match std::fs::remove_file(at) {
      Err(why) if why.kind() != std::io::ErrorKind::NotFound => left.push(RemovalLeft::NotRemoved {
        path,
        reason: why.to_string(),
      }),
      _ => removed_pictures.push(shown(receiver, at)),
    }
  }

  let mut removed_folders: Vec<Value> = vec![];

  for held in &folders {
    let path = shown(receiver, &held.path);

    if !under(receiver, &held.path) {
      left.push(RemovalLeft::OutsideDataRoot { path });
      continue;
    }

    match held.remove(receiver).await {
      Ok(()) => removed_folders.push(json!({ "what": held.what, "path": path })),
      Err(reason) => left.push(RemovalLeft::NotRemoved { path, reason }),
    }
  }

  let units = holders(receiver, room).await;

  receiver.news().room_removed(room);
  eprintln!("bmo-receiver: the room {room}'s registration was removed, as {by} asked");

  let (goes, stays) = split(&counted);

  Ok(json!({
    "room": room,
    "taken": goes,
    "cleared": stays,
    "pictures": removed_pictures,
    "settings": record.settings.taken,
    "folders": removed_folders,
    "holders": units,
    "units": json!({ "agency": unit_answer(agency) }),
    "couldNot": left,
  }))
}

// Whether a path may be taken down: only under this data root.
fn under(receiver: &Receiver, path: &Path) -> bool {
  path.is_absolute() && path.starts_with(receiver.data_root().path())
}

// A database that did not answer the removal.
fn failed(why: sqlx::Error) -> Refusal {
  store(Fault::new(why.to_string()))
}

/// A room's registration removed, its whole record with it; with `dryRun`, what the removal would do and nothing
/// written.
pub async fn remove(State(served): State<Served>, raw: RawQuery, writable: WritableRoom) -> Answered {
  let room = writable.room;
  let asked = super::query::<RemovalQuery>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let by = asked.by.trim();

  administrator(&receiver, by).await?;

  if asked.dry_run {
    return answered(shown_removal(&receiver, room).await?);
  }

  if let Some(refusal) = refusals(&Facing {
    receiver: &receiver,
    room,
  })
  .await
  .unwrap_or_default()
  .first()
  {
    return Err(code(refusal));
  }

  answered(carry(&receiver, room, by).await?)
}

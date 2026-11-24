//! An account's removal, the platform's permanent delete as the receiver carries its part (docs/receiver.md, 「An
//! account's removal」; docs/api.md, 「Rooms」): what it would take, counted with nothing written, and the removal
//! itself. Each private room of the account goes through the room's own removal; the rest is the account's rows.
//! An organisation unit's removal takes nothing here, since the receiver keys nothing by one. Every route is the
//! administrator's alone, under the console's token.

use std::path::Path as FilePath;

use axum::extract::{Extension, Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use super::room_removal::{self, Unfinished};
use super::{Answered, Credential, Refusal, Served, answered, code, refused, store, unit};
use crate::accounts::{self, AccountRows};
use crate::presence::taken::console_presence;
use crate::receiver::{Receiver, agent_pictures};

/// What an organisation unit's removal is asked beside its number: the unit's company, which the receiver keys
/// nothing by.
#[derive(Deserialize, Default)]
pub struct UnitAsked {
  #[serde(default)]
  company: Option<String>,
}

fn forbidden() -> Refusal {
  refused(
    StatusCode::FORBIDDEN,
    "forbidden",
    "An account's or a unit's removal is the administrator's alone, under the console's token.",
  )
}

// The administrator's standing on the call: the console's token, `Owner-Role` naming the administrator and an
// `Owner-Key` the web unit answers as the administrator. Answers the key, the one the removal is logged as asked by.
async fn administrator(
  receiver: &Receiver,
  credential: Credential,
  headers: &HeaderMap,
) -> Result<String, Refusal> {
  let header = |name: &str| {
    headers
      .get(name)
      .and_then(|value| value.to_str().ok())
      .map(str::trim)
      .unwrap_or_default()
  };
  let key = header("owner-key");

  if credential != Credential::Console || header("owner-role") != "administrator" || key.is_empty() {
    return Err(forbidden());
  }

  match receiver.units().web.user(key).await {
    Ok(user) if user.administrator => Ok(key.to_owned()),
    Ok(_) => Err(forbidden()),
    Err(why) if why.status().is_some_and(|status| (400..500).contains(&status)) => Err(forbidden()),
    Err(why) => Err(unit(why)),
  }
}

// A positive decimal number as a path or a query gives it.
fn positive(text: &str, what: &str) -> Result<i64, Refusal> {
  let digits = !text.is_empty() && text.bytes().all(|digit| digit.is_ascii_digit());

  digits
    .then(|| text.parse::<i64>().ok())
    .flatten()
    .filter(|number| *number > 0)
    .ok_or_else(|| {
      refused(StatusCode::BAD_REQUEST, "bad-request", &format!("{what} is a positive decimal number"))
    })
}

// Every kind an account's removal names, each with its number.
fn kinds(private_rooms: i64, rows: AccountRows) -> Value {
  json!({
    "private-rooms": private_rooms,
    "memberships": rows.memberships,
    "identities": rows.identities,
    "names": rows.names,
    "mutes": rows.mutes,
    "pictures": rows.pictures,
  })
}

/// `GET /accounts/{account}/removal`: what the account's removal would take now, by kind; nothing written.
pub async fn counted(
  State(served): State<Served>,
  Extension(credential): Extension<Credential>,
  headers: HeaderMap,
  Path(account): Path<String>,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  administrator(&receiver, credential, &headers).await?;

  let account = positive(&account, "an account")?;
  let rooms = accounts::kept_private_rooms_of(receiver.db(), account).await.map_err(store)?;
  let rows = accounts::account_rows(receiver.db(), account).await.map_err(store)?;

  answered(json!({ "counts": kinds(i64::try_from(rooms.len()).unwrap_or(i64::MAX), rows) }))
}

// A refusal that stopped the removal at one of the account's private rooms, its sentence naming the room.
fn stopped_at(room: i64, why: Refusal) -> Refusal {
  Refusal {
    error: format!("The account's private room {room} was not removed whole: {}", why.error),
    ..why
  }
}

// One private room of the account removed as the administrator's removal of a room removes it: refused where a
// declaration refuses it, and not begun where the agency does not answer for it.
async fn private_room_removed(receiver: &Receiver, room: i64, by: &str) -> Result<(), Refusal> {
  if let Some(refusal) = room_removal::refusal(receiver, room).await {
    return Err(stopped_at(room, code(&refusal)));
  }

  if let Err(why) = receiver.units().agency.forget_room(room, true).await {
    return Err(stopped_at(room, unit(why)));
  }

  let carried = room_removal::carry(receiver, room, by).await.map_err(|why| stopped_at(room, why))?;

  match carried.unfinished {
    None => Ok(()),
    Some(Unfinished::Agency(why)) => {
      Err(stopped_at(room, refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why)))
    }
    Some(Unfinished::Left(left)) => {
      let left: Vec<String> = left.iter().map(ToString::to_string).collect();

      Err(stopped_at(room, refused(StatusCode::INTERNAL_SERVER_ERROR, "failed", &left.join("; "))))
    }
  }
}

// The files of the pictures set on the identity taken down, only under this data root and never one an agent's
// picture holds: what could not be.
async fn pictures_taken_down(receiver: &Receiver, files: Vec<String>) -> Vec<String> {
  let mut left = vec![];

  for path in files {
    let at = FilePath::new(&path);

    if !at.is_absolute() || !at.starts_with(receiver.data_root().path()) {
      left.push(format!("{path}: outside this data root"));
      continue;
    }

    if agent_pictures::held_by_agent(receiver.db(), &path).await.unwrap_or(true) {
      continue;
    }

    if let Err(why) = tokio::fs::remove_file(at).await
      && why.kind() != std::io::ErrorKind::NotFound
    {
      left.push(format!("{path}: {why}"));
    }
  }

  left
}

/// `DELETE /accounts/{account}`: each of the account's private rooms removed whole, then its memberships of every
/// other room and its record identity taken; what it took, by kind. A room that stops it leaves what was taken
/// before taken, and calling again goes on from there.
pub async fn remove(
  State(served): State<Served>,
  Extension(credential): Extension<Credential>,
  headers: HeaderMap,
  Path(account): Path<String>,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let by = administrator(&receiver, credential, &headers).await?;
  let account = positive(&account, "an account")?;
  let rooms = accounts::kept_private_rooms_of(receiver.db(), account).await.map_err(store)?;

  for room in &rooms {
    private_room_removed(&receiver, *room, &by).await?;
  }

  // The console's word on the account's presence is taken back while its rooms still name it.
  console_presence(&receiver, account, None).await.map_err(store)?;

  let (rows, files) = accounts::take_account_rows(receiver.db(), account).await.map_err(store)?;
  let left = pictures_taken_down(&receiver, files).await;

  if rows.pictures > 0 {
    receiver.news().publish(&crate::events::avatars());
  }

  eprintln!("bmo-receiver: account {account}'s rooms and rows were removed, as {by} asked");

  if !left.is_empty() {
    return Err(refused(
      StatusCode::INTERNAL_SERVER_ERROR,
      "failed",
      &format!("The account's pictures were taken but their files stand: {}", left.join("; ")),
    ));
  }

  answered(json!({ "removed": kinds(i64::try_from(rooms.len()).unwrap_or(i64::MAX), rows) }))
}

// An organisation unit as the path and the query name it, refused where either is not a positive number.
fn unit_named(unit: &str, raw: RawQuery) -> Result<(), Refusal> {
  let asked = super::query::<UnitAsked>(raw)?;

  positive(unit, "an organisation unit")?;

  if let Some(company) = asked.company.as_deref() {
    positive(company, "company")?;
  }

  Ok(())
}

/// `GET /org-units/{unit}/removal`: nothing, since the receiver keys nothing by an organisation unit.
pub async fn unit_counted(
  State(served): State<Served>,
  Extension(credential): Extension<Credential>,
  headers: HeaderMap,
  Path(unit): Path<String>,
  raw: RawQuery,
) -> Answered {
  administrator(&served.receiver, credential, &headers).await?;
  unit_named(&unit, raw)?;

  answered(json!({ "counts": {} }))
}

/// `DELETE /org-units/{unit}`: nothing taken, since the receiver keys nothing by an organisation unit.
pub async fn unit_remove(
  State(served): State<Served>,
  Extension(credential): Extension<Credential>,
  headers: HeaderMap,
  Path(unit): Path<String>,
  raw: RawQuery,
) -> Answered {
  administrator(&served.receiver, credential, &headers).await?;
  unit_named(&unit, raw)?;

  answered(json!({ "removed": {} }))
}

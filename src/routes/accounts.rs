//! The console's accounts in its rooms (docs/api.md, 「Rooms」 and 「Sends」; docs/receiver.md, 「Console rooms and
//! accounts」): an account's record identity, the rooms it is a member of, a room's members by account let in and
//! taken out with a line saying so, several or an organisation unit's members let in with one line for them all, and an account's words and files said in a room, kept under the owner's sender
//! for the owner's account and under the account's record identity for any other. The receiver judges no right of an
//! account here; it holds only that nothing in a private room is said or changed by another account.

use axum::{
  Json,
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use futures_util::future::try_join_all;
use receiver_codes::{FixedLine, RoomAccountRefusal};
use serde::Deserialize;
use serde_json::{Value, json};

use super::say::{Speaking, files_form, files_said, said, sendable, speakable};
use super::writable::WritableRoom;
use super::{Answered, Refusal, Served, answered, code, query, read, refused, store};
use crate::accounts::{self, Member, Privacy, Speaker};
use crate::receiver::{NotReceived, Receiver};
use crate::roads::providers::SaidBy;
use crate::routes::say::known;

/// Words an account said in the console.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Words {
  #[serde(default)]
  account: Option<i64>,
  #[serde(default)]
  name: Option<String>,
  text: String,
  #[serde(default)]
  reply_to: Option<Value>,
}

fn reached(why: NotReceived) -> Refusal {
  match why {
    NotReceived::Store(why) => store(why),
    why => refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string()),
  }
}

fn member_shape(member: &Member) -> Value {
  json!({ "account": member.account, "addedBy": member.added_by, "addedAt": member.added_at })
}

/// An account's key as the path gives it, decimal text.
pub(super) fn account_key(account: &str) -> Result<i64, Refusal> {
  account.trim().parse::<i64>().map_err(|_| {
    refused(StatusCode::BAD_REQUEST, "bad-request", "an account is named by its key, a number")
  })
}

/// A room the receiver holds, refused 404 where it holds none.
pub(super) async fn held(receiver: &Receiver, room: i64) -> Result<(), Refusal> {
  if known(receiver, room).await? {
    return Ok(());
  }

  Err(refused(
    StatusCode::NOT_FOUND,
    "not-found",
    &format!("The receiver holds no room {room}."),
  ))
}

// The account a room is private to, where it is a private room.
async fn private_to(receiver: &Receiver, room: i64) -> Result<Option<i64>, Refusal> {
  accounts::private_account(receiver, room).await.map_err(reached)
}

/// What `PUT /accounts/{account}/identity` may carry: the name the identity goes by from then.
#[derive(Default, Deserialize)]
pub struct Naming {
  #[serde(default)]
  name: Option<String>,
}

/// `PUT /accounts/{account}/identity`: the account's record identity, made where it does not stand, and named where
/// the body gives a name; a missing body is an empty one.
pub async fn identity(State(served): State<Served>, Path(account): Path<String>, body: Bytes) -> Answered {
  let account = account_key(&account)?;
  let asked = if body.iter().all(u8::is_ascii_whitespace) {
    Naming::default()
  } else {
    read::<Naming>(&body)?
  };
  let name = match asked.name.as_deref().map(str::trim) {
    Some("") => {
      return Err(refused(
        StatusCode::BAD_REQUEST,
        "bad-request",
        "name, where given, is a non-empty string",
      ));
    }
    given => given,
  };
  let sender = accounts::identity(served.receiver.db(), account).await.map_err(store)?;

  if let Some(name) = name {
    accounts::name_identity(served.receiver.db(), sender, name).await.map_err(store)?;
  }

  answered(json!({ "account": account, "sender": sender }))
}

/// `GET /accounts/{account}/rooms`: the rooms the account is a member of now, its private rooms among them.
pub async fn rooms(State(served): State<Served>, Path(account): Path<String>) -> Answered {
  let account = account_key(&account)?;
  let receiver: Receiver = (*served.receiver).clone();
  let mut rooms = accounts::private_rooms_of(&receiver, account).await.map_err(reached)?;

  rooms.extend(accounts::rooms_of(receiver.db(), account).await.map_err(store)?);
  rooms.sort_unstable();
  rooms.dedup();

  answered(json!({ "rooms": rooms }))
}

/// `GET /rooms/{roomId}/accounts`: the room's members by account; a private room's own account alone.
pub async fn members(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  held(&receiver, room).await?;

  let members = match private_to(&receiver, room).await? {
    Some(account) => vec![Member {
      account,
      added_by: None,
      added_at: None,
    }],
    None => accounts::members(receiver.db(), room).await.map_err(store)?,
  };

  answered(json!({ "accounts": members.iter().map(member_shape).collect::<Vec<Value>>() }))
}

/// What `GET /rooms/{roomId}/audience` asks: the supervisor the room's audience is read for.
#[derive(Deserialize)]
struct AudienceAsk {
  agent: String,
}

/// `GET /rooms/{roomId}/audience`: whether everyone in the room stands in the company of the supervisor's owner
/// (docs/receiver.md, 「Whether someone outside the company sits in a room」).
pub async fn audience(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<AudienceAsk>(raw)?;
  let agent = asked.agent.trim();
  let receiver: Receiver = (*served.receiver).clone();

  if agent.is_empty() {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      "agent (a supervisor's name) is required",
    ));
  }

  held(&receiver, room).await?;

  let audience = crate::audience::audience(&receiver, room, agent).await;

  answered(json!({ "room": room, "agent": agent, "audience": audience }))
}

// A room whose members can be changed: one the receiver holds that is no one's private room, sealed ones among
// them.
async fn changeable(receiver: &Receiver, room: i64, by: i64) -> Result<(), Refusal> {
  held(receiver, room).await?;

  if accounts::privacy(receiver, room).await.map_err(reached)? != Privacy::Shared {
    return Err(code(&RoomAccountRefusal::PrivateRoom { room, account: by }));
  }

  Ok(())
}

// A line the room hears of a member's change, said by the receiver itself and kept as the fixed line it is; a room
// that takes no line is said in the journal instead, and the change stands either way.
async fn told(receiver: &Receiver, room: i64, line: FixedLine) {
  if let Err(why) = receiver.speech().say_line(room, &line).await {
    println!("chat {room}: a member's change went unsaid: {why}");
  }
}

/// The account a write acts for, refused where the call names none.
pub(super) fn by_of(writable: &WritableRoom) -> Result<i64, Refusal> {
  writable
    .writer
    .account()
    .ok_or_else(|| refused(StatusCode::BAD_REQUEST, "bad-request", "by (an account) is required"))
}

/// What `POST /rooms/{roomId}/accounts` carries beside `by`: the accounts to let in.
#[derive(Deserialize)]
pub struct Several {
  #[serde(default)]
  accounts: Vec<i64>,
}

/// What `POST /rooms/{roomId}/unit-accounts` carries beside `by`: the organisation unit whose people are let in.
#[derive(Deserialize)]
pub struct FromUnit {
  unit: i64,
}

// Every account let in by `by` as `PUT` lets one in, in the order given: each as a member as it now stands, and the
// keys of those this call let in.
async fn all_let_in(
  receiver: &Receiver,
  room: i64,
  accounts: &[i64],
  by: i64,
) -> Result<(Vec<Value>, Vec<i64>), Refusal> {
  let mut members = vec![];
  let mut let_in = vec![];

  for &account in accounts {
    let (member, now) = accounts::let_in(receiver.db(), room, account, by).await.map_err(store)?;

    members.push(member_shape(&member));

    if now {
      let_in.push(account);
    }
  }

  Ok((members, let_in))
}

/// `POST /rooms/{roomId}/accounts`: several accounts let in by `by`, with one line naming those it let in; a member
/// already one stands as it is, and a call that let nobody in writes nothing.
pub async fn let_in_several(State(served): State<Served>, writable: WritableRoom) -> Answered {
  let room = writable.room;
  let by = by_of(&writable)?;
  let asked = read::<Several>(writable.writer.body())?;
  let mut seen = asked.accounts.clone();

  seen.sort_unstable();
  seen.dedup();

  if asked.accounts.is_empty() || seen.len() != asked.accounts.len() {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      "accounts names at least one account, each once",
    ));
  }

  let receiver: Receiver = (*served.receiver).clone();

  changeable(&receiver, room, by).await?;

  let (members, let_in) = all_let_in(&receiver, room, &asked.accounts, by).await?;
  let line = match let_in.as_slice() {
    [] => None,
    [account] => Some(FixedLine::AccountLetIn {
      account: *account,
      by,
    }),
    _ => Some(FixedLine::AccountsLetIn {
      accounts: let_in.clone(),
      by,
    }),
  };

  if let Some(line) = line {
    told(&receiver, room, line).await;
  }

  answered(json!({ "accounts": members, "letIn": let_in }))
}

// The refusal of a call the web unit did not answer.
fn upstream(why: impl std::fmt::Display) -> Refusal {
  refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string())
}

// The people of a unit whose accounts have not left, in the unit's order: the web unit keeps an account that has
// left in its unit and marks the departure on the account's machine standing, never in the unit's answer.
async fn not_left(receiver: &Receiver, people: &[i64]) -> Result<Vec<i64>, Refusal> {
  let web = &receiver.units().web;
  let nodes = try_join_all(people.iter().map(|account| async move {
    let user = account.to_string();

    web.node_of(&user).await
  }))
  .await
  .map_err(upstream)?;

  Ok(
    people
      .iter()
      .zip(nodes)
      .filter(|(_, node)| !node.left)
      .map(|(account, _)| *account)
      .collect(),
  )
}

/// `POST /rooms/{roomId}/unit-accounts`: the people of an organisation unit who have not left, as the web unit
/// answers them at this moment, let in by `by` with one line naming the unit; the room keeps no link to the unit.
pub async fn let_in_unit(State(served): State<Served>, writable: WritableRoom) -> Answered {
  let room = writable.room;
  let by = by_of(&writable)?;
  let asked = read::<FromUnit>(writable.writer.body())?;
  let receiver: Receiver = (*served.receiver).clone();

  changeable(&receiver, room, by).await?;

  let unit = match receiver.units().web.organisation_unit(asked.unit).await {
    Ok(Some(unit)) => unit,
    Ok(None) => {
      return Err(refused(
        StatusCode::NOT_FOUND,
        "not-found",
        &format!("The web unit holds no organisation unit {}.", asked.unit),
      ));
    }
    Err(why) => return Err(upstream(why)),
  };
  let people = not_left(&receiver, &unit.people).await?;
  let (members, let_in) = all_let_in(&receiver, room, &people, by).await?;

  if !let_in.is_empty() {
    let line = FixedLine::UnitLetIn {
      unit: unit.id,
      unit_name: unit.name.clone(),
      accounts: let_in.clone(),
      by,
    };

    told(&receiver, room, line).await;
  }

  answered(json!({ "unit": unit.id, "unitName": unit.name, "accounts": members, "letIn": let_in }))
}

/// `PUT /rooms/{roomId}/accounts/{account}`: an account let in by `by`, with a line saying so; the member as it
/// stands where it already is one.
pub async fn let_in(
  State(served): State<Served>,
  Path((_, account)): Path<(i64, String)>,
  writable: WritableRoom,
) -> Answered {
  let room = writable.room;
  let account = account_key(&account)?;
  let by = by_of(&writable)?;
  let receiver: Receiver = (*served.receiver).clone();

  changeable(&receiver, room, by).await?;

  let (member, now) = accounts::let_in(receiver.db(), room, account, by).await.map_err(store)?;

  if now {
    told(&receiver, room, FixedLine::AccountLetIn { account, by }).await;
  }

  answered(member_shape(&member))
}

/// `DELETE /rooms/{roomId}/accounts/{account}?by=`: a member taken out by `by`, leaving where `by` is itself, with a
/// line saying so; nothing where it is no member.
pub async fn take_out(
  State(served): State<Served>,
  Path((_, account)): Path<(i64, String)>,
  writable: WritableRoom,
) -> Result<StatusCode, Refusal> {
  let room = writable.room;
  let account = account_key(&account)?;
  let by = by_of(&writable)?;
  let receiver: Receiver = (*served.receiver).clone();

  changeable(&receiver, room, by).await?;

  if accounts::take_out(receiver.db(), room, account, by).await.map_err(store)? {
    let line = if by == account {
      FixedLine::AccountLeft { account }
    } else {
      FixedLine::AccountTakenOut { account, by }
    };

    told(&receiver, room, line).await;
  }

  Ok(StatusCode::NO_CONTENT)
}

/// Who an account speaks as in a room: the owner on his own sender for the owner's account, and the account under
/// its record identity and the shown name the console gives for any other, which the identity goes by from then.
pub(super) async fn speaker(
  receiver: &Receiver,
  room: i64,
  account: i64,
  name: &str,
) -> Result<Speaking, Refusal> {
  Ok(match accounts::speaker(receiver, account).await.map_err(reached)? {
    Speaker::Owner => Speaking::owner(receiver, room).await,
    Speaker::Account { sender } => {
      if !name.is_empty() {
        accounts::name_identity(receiver.db(), sender, name).await.map_err(store)?;
      }

      Speaking {
        sender_id: sender,
        name: name.to_owned(),
        said_by: SaidBy::Account {
          account,
          name: name.to_owned(),
        },
      }
    }
  })
}

// The account and its shown name a call names, refused where either is missing.
fn named(account: Option<i64>, name: Option<&str>) -> Result<(i64, String), Refusal> {
  let name = name.map(str::trim).filter(|name| !name.is_empty());

  match (account, name) {
    (Some(account), Some(name)) => Ok((account, name.to_owned())),
    _ => Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      "account (a number) and name (a non-empty string) are required",
    )),
  }
}

/// `POST /rooms/{roomId}/words`: words an account said in the console, kept and carried as any message.
pub async fn words(
  State(served): State<Served>,
  writable: WritableRoom,
) -> Result<(StatusCode, Json<Value>), Refusal> {
  let room = writable.room;
  let asked = read::<Words>(writable.writer.body())?;
  let (account, name) = named(asked.account, asked.name.as_deref())?;
  let receiver: Receiver = (*served.receiver).clone();
  let text = speakable(&receiver, room, &asked.text).await?;
  let speaking = speaker(&receiver, room, account, &name).await?;
  let id = said(&receiver, room, &text, asked.reply_to.as_ref(), speaking).await?;

  Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

/// `POST /rooms/{roomId}/word-files`: files an account sent in the console, as `POST /owner/message-files` takes
/// them, kept and carried as any message.
pub async fn word_files(State(served): State<Served>, writable: WritableRoom) -> Answered {
  let room = writable.room;
  let receiver: Receiver = (*served.receiver).clone();
  let mut writer = writable.writer;
  let form = files_form(writer.form().await?, receiver.data_root()).await?;
  let (account, name) = named(form.account, form.name.as_deref())?;

  // The form names the account the request could not read off it, so the room is asked again for that account.
  writer.for_account(Some(account))?.room(&receiver, room).await?;
  sendable(&receiver, room, &form).await?;

  let speaking = speaker(&receiver, room, account, &name).await?;

  files_said(&receiver, room, form, speaking).await
}

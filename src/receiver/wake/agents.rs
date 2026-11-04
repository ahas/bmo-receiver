//! The designated agents of a room as the wake judges them (docs/wake.md, 「Who reaches the session」): who
//! answers there, by the room's own record, what each is called by and keeps a window for, and the number its
//! own words are filed under. The agents themselves are the agency's registrations, read through its port, so
//! an agent added as a row is judged the same way with nothing here naming it.

use std::time::Duration;

use serde_json::{Map, Value};

use receiver_ports::agency::{Agent, Agents};
use sea_orm::DatabaseConnection;

use super::asked;
use crate::receiver::{NotReceived, Receiver, Room};
use crate::secrets;
use crate::{all, now_ms, one, run};

/// One designated agent of a room: its registration name, the words a room calls it by, how long a room it
/// answered keeps it listening where it keeps a window at all, the account its registration names as its owner
/// where it names one, and whether the default answerer setting names it, so it answers the rooms that name
/// nobody of their own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Designated {
  pub name: String,
  pub call_names: Vec<String>,
  pub window_seconds: Option<i64>,
  pub owner_account: Option<i64>,
  pub default_answerer: bool,
}

impl Designated {
  /// The agent as its registration gives it: its label and the words its row names, held as one line
  /// separated by commas; `default` is the name the default answerer setting holds.
  pub fn of(agent: &Agent, default: &str) -> Self {
    let words = text(&agent.row, "aliases").unwrap_or_default();
    let named = agent.label.clone().into_iter().chain(words.split(',').map(str::to_owned));

    Self {
      name: agent.name.clone(),
      call_names: named.map(trim).filter(|word| !word.is_empty()).collect(),
      window_seconds: count(&agent.row, "windowSeconds"),
      owner_account: count(&agent.row, "ownerAccount"),
      default_answerer: agent.name == default,
    }
  }

  /// Whether the words call it: its label or one of its words stands in them, in either case. The secret marker is
  /// no word, so no name is found in it.
  pub fn called_by(&self, text: &str) -> bool {
    let lowered = secrets::unmarked(text).to_lowercase();

    self.call_names.iter().any(|name| lowered.contains(&name.to_lowercase()))
  }

  /// Whether it answers the rooms that name nobody.
  pub fn answers_unnamed(&self) -> bool {
    self.default_answerer
  }
}

fn trim(word: String) -> String {
  word.trim().to_owned()
}

fn text(row: &Map<String, Value>, key: &str) -> Option<String> {
  row.get(key).and_then(|value| value.as_str()).map(|word| word.to_owned())
}

fn count(row: &Map<String, Value>, key: &str) -> Option<i64> {
  match row.get(key) {
    Some(value) => value.as_i64().or_else(|| value.as_bool().map(|on| if on { 1 } else { 0 })),
    None => None,
  }
}

/// Every awake agent, as the agency registers it, by name.
pub async fn awake(receiver: &Receiver) -> Result<Vec<Designated>, NotReceived> {
  Ok(
    listed(receiver)
      .await?
      .iter()
      .filter(|(_, _, awake)| *awake)
      .map(|(_, designated, _)| designated.clone())
      .collect(),
  )
}

/// The agent as the agency registers it, awake or not, by name; none where no row names it.
pub async fn registered(receiver: &Receiver, name: &str) -> Result<Option<Designated>, NotReceived> {
  Ok(
    listed(receiver)
      .await?
      .iter()
      .find(|(agent, _, _)| agent == name)
      .map(|(_, designated, _)| designated.clone()),
  )
}

/// Every agent the agency registers, awake or not, each by name with its designated form and whether it is
/// awake.
async fn listed(receiver: &Receiver) -> Result<Vec<(String, Designated, bool)>, NotReceived> {
  let listed: Agents = receiver.agents().await.map_err(NotReceived::from)?;
  let default = receiver.default_answerer();
  let mut agents = listed
    .agents
    .iter()
    .map(|agent| (agent.name.clone(), Designated::of(agent, &default), agent.awake))
    .collect::<Vec<(String, Designated, bool)>>();

  agents.sort_by(|one, other| one.0.cmp(&other.0));

  Ok(agents)
}

/// The agent that answers the rooms that name nobody: the one the default answerer setting names, as the agency
/// registers it where it is awake, and otherwise by its name alone, called by nothing and keeping no window.
pub async fn default_designated(receiver: &Receiver) -> Result<Designated, NotReceived> {
  Ok(
    awake(receiver)
      .await?
      .iter()
      .find(|agent| agent.default_answerer)
      .cloned()
      .unwrap_or_else(|| Designated {
        name: receiver.default_answerer(),
        call_names: Vec::new(),
        window_seconds: None,
        owner_account: None,
        default_answerer: true,
      }),
  )
}

/// The awake agents a room records as its answerers; none where it names nobody.
pub async fn named_designated(receiver: &Receiver, chat_id: i64) -> Result<Vec<Designated>, NotReceived> {
  let awake = awake(receiver).await?;

  named_among(receiver.db(), chat_id, &awake).await
}

/// Of the awake agents given, those a room records as its answerers; none where it names nobody. A read that
/// walks many rooms reads the awake agents once and hands them to each room.
pub async fn named_among(
  db: &DatabaseConnection,
  chat_id: i64,
  awake: &[Designated],
) -> Result<Vec<Designated>, NotReceived> {
  let rows = all(
    db,
    r#"SELECT "supervisor" FROM "room_supervisors" WHERE "chat_id" = $1 ORDER BY "supervisor""#,
    vec![chat_id.into()],
  )
  .await?;
  let named: Vec<String> = rows
    .iter()
    .map(|row| row.try_get::<String>("", "supervisor"))
    .collect::<Result<_, _>>()?;

  Ok(awake.iter().filter(|agent| named.contains(&agent.name)).cloned().collect())
}

/// Who answers in a room, and whether one of them answers it alone: the awake agents the room records as its
/// answerers; where it records none, the supervisor the room's private person owns where one stands, and
/// otherwise the default answerer, which answers the owner's private chat and every room that is no one's own
/// chat alike. A room that records none is answered alone where it is the owner's private chat or a user's own
/// private chat.
pub async fn room_designated(
  receiver: &Receiver,
  chat_id: i64,
  room: Room,
) -> Result<(Vec<Designated>, bool), NotReceived> {
  let answerers = named_designated(receiver, chat_id).await?;

  if !answerers.is_empty() {
    let alone = answerers.len() == 1;

    return Ok((answerers, alone));
  }

  let private = one_to_one_user(receiver, room).await?;

  if let Some(user) = private.as_deref() {
    if let Some(owner) = owned_by(receiver, user).await? {
      return Ok((vec![owner], true));
    }

    said_fallback(chat_id, user);
  }

  let alone = room.owner_room || private.is_some();

  Ok((vec![default_designated(receiver).await?], alone))
}

/// The supervisor a room's own private person owns, where the room is a user's one-to-one with the road's own
/// voice and one of that user's supervisors is awake; none where the room is no one's own private chat or its
/// person owns none.
pub async fn private_answerer(
  receiver: &Receiver,
  chat_id: i64,
) -> Result<Option<Designated>, NotReceived> {
  let room = receiver.roads().room_facts(chat_id).await;
  let Some(user) = one_to_one_user(receiver, room).await? else {
    return Ok(None);
  };

  owned_by(receiver, &user).await
}

/// The user the account model links to the person a room is a one-to-one with, where its road says it is one and
/// the room is not the owner's private chat.
async fn one_to_one_user(receiver: &Receiver, room: Room) -> Result<Option<String>, NotReceived> {
  if room.owner_room {
    return Ok(None);
  }

  let Some(person) = room.private_to else {
    return Ok(None);
  };

  Ok(asked("whose user the sender is", receiver.units.web.user_of_sender(person).await).flatten())
}

/// The awake supervisor whose registration names `user` as its owner; the first by name where more than one
/// does.
async fn owned_by(receiver: &Receiver, user: &str) -> Result<Option<Designated>, NotReceived> {
  let Ok(account) = user.trim().parse::<i64>() else {
    return Ok(None);
  };

  Ok(awake(receiver).await?.into_iter().find(|agent| agent.owner_account == Some(account)))
}

// The rooms whose private person owned no awake supervisor, so the journal says it once for each.
static UNOWNED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<i64>>> =
  std::sync::LazyLock::new(Default::default);

/// The journal's line that a private room's person owns no awake supervisor, so the default answerer answers
/// the room as it did before, said once for the room.
fn said_fallback(chat_id: i64, user: &str) {
  let first = UNOWNED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(chat_id);

  if first {
    println!(
      "bmo-receiver: chat {chat_id}: the user {user} of the private room owns no awake supervisor, so the default answerer answers it"
    );
  }
}

/// Whether an agent answers in a room: a room that names it among its awake answerers; for a room that names
/// nobody, the supervisor its private person owns where one stands, and otherwise the default answerer.
pub async fn answers(receiver: &Receiver, agent: &str, chat_id: i64) -> Result<bool, NotReceived> {
  let named = named_designated(receiver, chat_id).await?;

  if named.is_empty() {
    if let Some(owner) = private_answerer(receiver, chat_id).await? {
      return Ok(owner.name == agent);
    }

    return Ok(default_designated(receiver).await?.name == agent);
  }

  Ok(named.iter().any(|one| one.name == agent))
}

/// The first agent a room records as its answerer, where it records one; none where it records nobody. The
/// instruction a job's status notice queues goes to this agent, then to the requester's private room, then to the
/// default answerer (docs/api.md, 「Instructions」).
pub async fn answerer(receiver: &Receiver, chat_id: i64) -> Result<Option<String>, NotReceived> {
  Ok(named_designated(receiver, chat_id).await?.first().map(|agent| agent.name.clone()))
}

/// The agent that says the system's own lines in a room (docs/voiced-notices.md, 「The rule」): the first
/// supervisor the room names where it names none of the default answerer's, the supervisor a private room's
/// person owns where one stands, and otherwise the default answerer.
pub async fn voice_of(receiver: &Receiver, chat_id: i64) -> Result<String, NotReceived> {
  let default = default_designated(receiver).await?;
  let named: Vec<String> = named_designated(receiver, chat_id)
    .await?
    .iter()
    .map(|agent| agent.name.clone())
    .collect();

  if let Some(first) = named.first() {
    return Ok(if named.contains(&default.name) { default.name } else { first.clone() });
  }

  Ok(match private_answerer(receiver, chat_id).await? {
    Some(owner) => owner.name,
    None => default.name,
  })
}

/// The sender number an agent's words are filed under in a room. For the default answerer, the road's own voice,
/// it is who the room's road says speaks there, whichever road carried the message: an answer said on another
/// road there is filed under the number that road gave the agent, not the one a console gave it for its own
/// rooms. Every other agent's, and the default answerer's where the room's road names nobody, is the number the
/// road gave it.
pub async fn own_number(
  receiver: &Receiver,
  agent: &Designated,
  via: &str,
  identity: Option<i64>,
) -> Result<Option<i64>, NotReceived> {
  if agent.answers_unnamed() && identity.is_some() {
    return Ok(identity);
  }

  let given = one(
    receiver.db(),
    r#"SELECT "sender_id" FROM "provider_users" WHERE "provider" = $1 AND "user_id" = $2"#,
    vec![via.into(), agent.name.clone().into()],
  )
  .await?;

  Ok(given.map(|row| row.try_get("", "sender_id")).transpose()?)
}

/// The sender an agent's newest own words in a room's record are archived under, whatever road carried them; none
/// where the record holds no words of the agent's there.
pub async fn archived_number(
  db: &DatabaseConnection,
  chat_id: i64,
  agent: &str,
) -> Result<Option<i64>, NotReceived> {
  let row = one(
    db,
    r#"SELECT "sender_id" FROM "messages" WHERE "chat_id" = $1 AND "agent_name" = $2 AND "sender_id" IS NOT NULL
       ORDER BY "sent_at" DESC, "message_id" DESC LIMIT 1"#,
    vec![chat_id.into(), agent.into()],
  )
  .await?;

  Ok(row.map(|row| row.try_get("", "sender_id")).transpose()?)
}

/// Whether a message of a room's record holds an agent's own words (docs/wake.md, 「Who reaches the session」): one
/// kept as a send of the agent's, or one filed under `own`, the number its words stand under.
pub async fn holds_own(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  agent: &str,
  own: Option<i64>,
) -> Result<bool, NotReceived> {
  let row = one(
    db,
    r#"SELECT 1 AS "one" FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2
       AND ("agent_name" = $3 OR "sender_id" = $4)"#,
    vec![chat_id.into(), message_id.into(), agent.into(), own.into()],
  )
  .await?;

  Ok(row.is_some())
}

/// When an agent's window in a room closes, in epoch milliseconds; zero where none was opened.
pub async fn window_until(db: &DatabaseConnection, chat_id: i64, agent: &str) -> Result<i64, NotReceived> {
  let row = one(
    db,
    r#"SELECT "open_until" FROM "room_windows" WHERE "chat_id" = $1 AND "agent" = $2"#,
    vec![chat_id.into(), agent.into()],
  )
  .await?;

  Ok(row.map(|row| row.try_get("", "open_until")).transpose()?.unwrap_or(0))
}

/// An agent's window in a room watched to its end: once `until` passes, where it still stands as the window's end,
/// the room's screens are told the window closed then (docs/wake.md, 「The names and the window」). An end moved
/// meanwhile is another end, watched by whoever moved it.
pub fn watch_window(receiver: &Receiver, chat_id: i64, agent: &str, until: i64) {
  let receiver = receiver.clone();
  let agent = agent.to_owned();

  tokio::spawn(async move {
    tokio::time::sleep(Duration::from_millis(u64::try_from(until - now_ms()).unwrap_or(0))).await;

    if window_until(receiver.db(), chat_id, &agent).await.ok() == Some(until) {
      receiver.news().window_closed(chat_id, &agent, until);
    }
  });
}

/// Every window end the database holds still to come, watched as the receiver starts.
pub async fn watch_windows(receiver: &Receiver) -> Result<(), NotReceived> {
  let rows = all(
    receiver.db(),
    r#"SELECT "chat_id", "agent", "open_until" FROM "room_windows" WHERE "open_until" > $1"#,
    vec![now_ms().into()],
  )
  .await?;

  for row in rows {
    watch_window(
      receiver,
      row.try_get("", "chat_id")?,
      &row.try_get::<String>("", "agent")?,
      row.try_get("", "open_until")?,
    );
  }

  Ok(())
}

/// An agent's window in a room opened until `until`, in epoch milliseconds; a window already open longer keeps
/// its own end.
pub async fn open_window(
  db: &DatabaseConnection,
  chat_id: i64,
  agent: &str,
  until: i64,
) -> Result<(), NotReceived> {
  Ok(
    run(
      db,
      r#"INSERT INTO "room_windows" ("chat_id", "agent", "open_until") VALUES ($1, $2, $3)
         ON CONFLICT ("chat_id", "agent") DO UPDATE SET "open_until" = GREATEST("room_windows"."open_until", excluded."open_until")"#,
      vec![chat_id.into(), agent.into(), until.into()],
    )
      .await
      .map(drop)?,
  )
}

/// An agent's window in a room moved to its own end at `until`, in epoch milliseconds; zero closes it. A turn of the
/// agent's sets the end it read, whatever was written before, so a window it did not open is not closed by it.
pub async fn set_window(
  db: &DatabaseConnection,
  chat_id: i64,
  agent: &str,
  until: i64,
) -> Result<(), NotReceived> {
  let written = if until <= 0 {
    run(
      db,
      r#"DELETE FROM "room_windows" WHERE "chat_id" = $1 AND "agent" = $2"#,
      vec![chat_id.into(), agent.into()],
    )
    .await
  } else {
    run(
      db,
      r#"INSERT INTO "room_windows" ("chat_id", "agent", "open_until") VALUES ($1, $2, $3)
         ON CONFLICT ("chat_id", "agent") DO UPDATE SET "open_until" = excluded."open_until""#,
      vec![chat_id.into(), agent.into(), until.into()],
    )
    .await
  };

  Ok(written.map(drop)?)
}

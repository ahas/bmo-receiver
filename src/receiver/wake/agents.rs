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
use crate::{all, now_ms, one, run};

/// One designated agent of a room: its registration name, the words a room calls it by, how long a room it
/// answered keeps it listening where it keeps a window at all, and whether it answers the rooms that name
/// nobody.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Designated {
  pub name: String,
  pub call_names: Vec<String>,
  pub window_seconds: Option<i64>,
  pub default_answerer: bool,
  /// How the room's roster groups and names it, as its registration says: `chat`, `worker` or another kind.
  pub kind: Option<String>,
}

impl Designated {
  /// The agent as its registration gives it: its label and the words its row names, held as one line
  /// separated by commas.
  pub fn of(agent: &Agent) -> Self {
    let words = text(&agent.row, "aliases").unwrap_or_default();
    let named = agent.label.clone().into_iter().chain(words.split(',').map(str::to_owned));

    Self {
      name: agent.name.clone(),
      call_names: named.map(trim).filter(|word| !word.is_empty()).collect(),
      window_seconds: count(&agent.row, "windowSeconds"),
      default_answerer: flag(&agent.row, "defaultAnswerer"),
      kind: text(&agent.row, "kind"),
    }
  }

  /// Whether it answers beside the chat agent rather than as it: any agent that is not the default answerer.
  pub fn beside(&self) -> bool {
    !self.default_answerer
  }

  /// Whether the words call it: its label or one of its words stands in them, in either case.
  pub fn called_by(&self, text: &str) -> bool {
    let lowered = text.to_lowercase();

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

fn flag(row: &Map<String, Value>, key: &str) -> bool {
  row
    .get(key)
    .is_some_and(|value| value.as_bool().unwrap_or_else(|| value.as_i64().is_some_and(|held| held != 0)))
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
  let mut agents = listed
    .agents
    .iter()
    .map(|agent| (agent.name.clone(), Designated::of(agent), agent.awake))
    .collect::<Vec<(String, Designated, bool)>>();

  agents.sort_by(|one, other| one.0.cmp(&other.0));

  Ok(agents)
}

/// The agent that answers the rooms that name nobody: the awake one its registration marks as the default,
/// and where none does, the deployment's own chat agent, called by nothing and keeping no window.
pub async fn default_designated(receiver: &Receiver) -> Result<Designated, NotReceived> {
  Ok(
    awake(receiver)
      .await?
      .iter()
      .find(|agent| agent.default_answerer)
      .cloned()
      .unwrap_or_else(|| Designated {
        name: receiver.deployment().agent.clone(),
        call_names: Vec::new(),
        window_seconds: None,
        default_answerer: true,
        kind: None,
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
    r#"SELECT "agent" FROM "room_answerers" WHERE "chat_id" = $1 ORDER BY "agent""#,
    vec![chat_id.into()],
  )
  .await?;
  let named: Vec<String> =
    rows.iter().map(|row| row.try_get::<String>("", "agent")).collect::<Result<_, _>>()?;

  Ok(awake.iter().filter(|agent| named.contains(&agent.name)).cloned().collect())
}

/// Who answers in a room, and whether one of them answers it alone: a room that records one answerer, or one
/// that records none and is a user's own private chat with the chat agent, the owner's or anyone else's, as the
/// room's road declares it, which is the chat agent's.
pub async fn room_designated(
  receiver: &Receiver,
  chat_id: i64,
  room: Room,
) -> Result<(Vec<Designated>, bool), NotReceived> {
  let answerers = named_designated(receiver, chat_id).await?;

  if answerers.is_empty() {
    let alone = room.owner_room || private_room(receiver, room.private_to).await?;

    return Ok((vec![default_designated(receiver).await?], alone));
  }

  let alone = answerers.len() == 1;

  Ok((answerers, alone))
}

/// Whether a room is a user's own private chat: its road says it is a one-to-one with the chat agent, with a
/// person whose sender the account model links to a user.
async fn private_room(receiver: &Receiver, private_to: Option<i64>) -> Result<bool, NotReceived> {
  match private_to {
    Some(person) => Ok(
      asked("whose user the sender is", receiver.units.web.user_of_sender(person).await)
        .flatten()
        .is_some(),
    ),
    None => Ok(false),
  }
}

/// Whether an agent answers in a room: a room that names it among its awake answerers, and for the default
/// answerer every room that names nobody awake.
pub async fn answers(receiver: &Receiver, agent: &str, chat_id: i64) -> Result<bool, NotReceived> {
  let named = named_designated(receiver, chat_id).await?;

  if named.is_empty() {
    return Ok(default_designated(receiver).await?.name == agent);
  }

  Ok(named.iter().any(|one| one.name == agent))
}

/// The agent that says the system's own lines in a room (docs/voiced-notices.md, 「The rule」).
pub async fn voice_of(receiver: &Receiver, chat_id: i64) -> Result<String, NotReceived> {
  let default = default_designated(receiver).await?;
  let named: Vec<String> = named_designated(receiver, chat_id)
    .await?
    .iter()
    .map(|agent| agent.name.clone())
    .collect();

  Ok(match named.first() {
    Some(first) if !named.contains(&default.name) => first.clone(),
    _ => default.name,
  })
}

/// The sender number an agent's words are filed under in a room. For the agent the road itself speaks as it is
/// who the room's road says the chat agent is there, whichever road carried the message: an answer said on
/// another road there is filed under the number that road gave the agent, not the one a console gave it for its
/// own rooms. Every other agent's, and the chat agent's where the room's road names nobody, is the number the
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

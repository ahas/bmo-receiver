//! The receiver's rows of an agent's reset (docs/wake.md, 「An agent's reset」): the agency takes its own part and
//! publishes the kind `supervisor-reset.created`, and the receiver takes its own rows of the
//! ranges the event names, in one write. The conversation range takes the whole record of each room the agent
//! answers alone, the run records those rooms' runs wrote down and those of the agent's own runs, and its own
//! instructions and windows in every room; the person range takes the marks its book of each room has read.

use futures_util::future::BoxFuture;
use receiver_ports::agency::Event;
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde::Deserialize;

use crate::hearing::Hearer;
use crate::receiver::Receiver;
use crate::{Fault, all, run};

/// The kind a reset that has taken the agency's part is published under.
pub const RESET: &str = "supervisor-reset.created";

/// The range of what the agent said: its conversations.
pub const CONVERSATION: &str = "conversation";

/// The range of what the agent remembers: its person book among it.
pub const PERSON: &str = "person";

/// The most run names one statement names, so a long list stays under what the database takes.
const RUNS_PER_WRITE: usize = 500;

/// A reset as the agency publishes it: the agent, the ranges taken, the rooms it answers alone, and the runs
/// its own log named before its lines went.
#[derive(Debug, Deserialize)]
pub struct Reset {
  pub agent: String,
  #[serde(default)]
  pub ranges: Vec<String>,
  #[serde(default)]
  pub alone: Vec<i64>,
  /// The runs the agent's own log lines went under: what a run wrote down is kept under the run's name, which
  /// says its room but not the agent that ran it.
  #[serde(default)]
  pub runs: Vec<String>,
}

/// What a reset took of the receiver's rows: each table it wrote, with the rows it took there.
pub type Taken = Vec<(&'static str, u64)>;

/// The reset as the service hears it on the agency's stream.
pub struct Resets {
  receiver: Receiver,
}

impl Resets {
  pub fn new(receiver: Receiver) -> Self {
    Resets { receiver }
  }
}

impl Hearer for Resets {
  fn kinds(&self) -> &'static [&'static str] {
    &[RESET]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      // A move of the point shares the kind and names no ranges; only a reset has rows of its own to take.
      if event.data.get("ranges").is_none() {
        return;
      }

      let reset = match serde_json::from_value::<Reset>(event.data.clone()) {
        Ok(reset) => reset,
        Err(why) => {
          eprintln!("bmo-receiver: an agent's reset that does not read: {why}");
          return;
        }
      };

      match take(&self.receiver, &reset, false).await {
        Ok(taken) => {
          let counts: Vec<String> = taken.iter().map(|(table, rows)| format!("{table} {rows}")).collect();

          println!("agent {}: its reset took {}", reset.agent, counts.join(", "));
        }
        Err(why) => eprintln!("agent {}: its reset's rows were not taken: {}", reset.agent, why.detail),
      }
    })
  }
}

/// The rooms of those named whose record holds the agent as its one answerer.
async fn answered_alone<C: ConnectionTrait>(db: &C, agent: &str, named: &[i64]) -> Result<Vec<i64>, Fault> {
  if named.is_empty() {
    return Ok(vec![]);
  }

  let rows = all(
    db,
    &format!(
      r#"SELECT "chat_id" FROM "room_supervisors" WHERE "chat_id" IN ({}) GROUP BY "chat_id"
         HAVING count(*) = 1 AND max("supervisor") = $1"#,
      numbered(2, named.len())
    ),
    std::iter::once(agent.into()).chain(named.iter().map(|room| (*room).into())).collect(),
  )
  .await?;

  rows.iter().map(|row| row.try_get::<i64>("", "chat_id").map_err(Fault::from)).collect()
}

// The placeholders `$from` onward, `count` of them, as an `IN` list writes them.
fn numbered(from: usize, count: usize) -> String {
  (from..from + count).map(|place| format!("${place}")).collect::<Vec<_>>().join(", ")
}

/// One table's rows gone by the condition given, or counted where `dry_run`.
async fn take_rows<C: ConnectionTrait>(
  db: &C,
  dry_run: bool,
  table: &str,
  condition: &str,
  values: Vec<sea_orm::Value>,
) -> Result<u64, Fault> {
  if dry_run {
    let rows =
      all(db, &format!(r#"SELECT count(*) AS "rows" FROM "{table}" WHERE {condition}"#), values).await?;
    let counted = rows
      .first()
      .map(|row| row.try_get::<i64>("", "rows"))
      .transpose()
      .map_err(Fault::from)?
      .unwrap_or(0);

    return Ok(u64::try_from(counted).unwrap_or(0));
  }

  run(db, &format!(r#"DELETE FROM "{table}" WHERE {condition}"#), values).await
}

/// The receiver's rows of the ranges the reset names, counted where `dry_run` and taken in one write otherwise,
/// answered with what each table gave up.
pub async fn take(receiver: &Receiver, reset: &Reset, dry_run: bool) -> Result<Taken, Fault> {
  let named = |range: &str| reset.ranges.iter().any(|one| one == range);
  let transaction = receiver.db().begin().await.map_err(Fault::from)?;
  let mut taken: Taken = vec![];

  if named(CONVERSATION) {
    let alone = answered_alone(&transaction, &reset.agent, &reset.alone).await?;

    if !alone.is_empty() {
      let rooms: Vec<sea_orm::Value> = alone.iter().map(|room| (*room).into()).collect();

      for table in ["messages", "chat_reads"] {
        let condition = format!(r#""chat_id" IN ({})"#, numbered(1, alone.len()));

        taken.push((table, take_rows(&transaction, dry_run, table, &condition, rooms.clone()).await?));
      }
    }

    // A chat run's name opens with its room, so a room's whole record goes by the name; the agent's own
    // runs in the rooms it shares are named by the agency's log, the only record that says which agent a run
    // ran for, and go by those names too.
    let mut runs = 0;

    if !alone.is_empty() {
      let condition = alone
        .iter()
        .enumerate()
        .map(|(at, _)| format!(r#""run" LIKE 'chat:' || CAST(${} AS TEXT) || ':%'"#, at + 1))
        .collect::<Vec<_>>()
        .join(" OR ");
      let rooms: Vec<sea_orm::Value> = alone.iter().map(|room| (*room).into()).collect();

      runs += take_rows(&transaction, dry_run, "run_records", &condition, rooms).await?;
    }

    for chunk in reset.runs.chunks(RUNS_PER_WRITE) {
      let condition = format!(r#""run" IN ({})"#, numbered(1, chunk.len()));
      let values: Vec<sea_orm::Value> = chunk.iter().map(|run| run.clone().into()).collect();

      runs += take_rows(&transaction, dry_run, "run_records", &condition, values).await?;
    }

    taken.push(("run_records", runs));

    for table in ["instructions", "room_windows"] {
      let condition = r#""agent" = $1"#;

      taken.push((
        table,
        take_rows(&transaction, dry_run, table, condition, vec![reset.agent.clone().into()]).await?,
      ));
    }
  }

  if named(PERSON) {
    taken.push((
      "people_reads",
      take_rows(
        &transaction,
        dry_run,
        "people_reads",
        r#""agent" = $1"#,
        vec![reset.agent.clone().into()],
      )
      .await?,
    ));
  }

  if dry_run {
    transaction.rollback().await.map_err(Fault::from)?;
  } else {
    transaction.commit().await.map_err(Fault::from)?;
  }

  Ok(taken)
}

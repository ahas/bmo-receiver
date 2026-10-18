//! The receiver's rows of a companion's reset (docs/wake.md, 「A companion's reset」; the agency's
//! `docs/companions.md`, 「Starting it afresh」): the agency takes its own part and publishes `companion-reset`, and
//! the receiver takes its own rows of the ranges it names, in one write. The conversation range takes the whole
//! record of each room the companion answers alone, and its own instructions and windows in every room; the person
//! range takes the marks its book of each room has read.

use futures_util::future::BoxFuture;
use receiver_ports::agency::Event;
use sea_orm::{ConnectionTrait, TransactionTrait};
use serde::Deserialize;

use crate::hearing::Hearer;
use crate::receiver::Receiver;
use crate::{Fault, all, run};

/// The event a reset that has taken the agency's part is published as.
pub const RESET: &str = "companion-reset";

/// The range of what the companion said: its conversations.
pub const CONVERSATION: &str = "conversation";

/// The range of what the companion remembers: its person book among it.
pub const PERSON: &str = "person";

/// A reset as the agency publishes it: the companion, the ranges taken, and the rooms it answers alone.
#[derive(Debug, Deserialize)]
pub struct Reset {
  pub agent: String,
  #[serde(default)]
  pub ranges: Vec<String>,
  #[serde(default)]
  pub alone: Vec<i64>,
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
      let reset = match serde_json::from_value::<Reset>(event.data.clone()) {
        Ok(reset) => reset,
        Err(why) => {
          eprintln!("bmo-receiver: a companion reset that does not read: {why}");
          return;
        }
      };

      match take(&self.receiver, &reset).await {
        Ok(taken) => {
          let counts: Vec<String> = taken.iter().map(|(table, rows)| format!("{table} {rows}")).collect();

          println!("companion {}: its reset took {}", reset.agent, counts.join(", "));
        }
        Err(why) => eprintln!("companion {}: its reset's rows were not taken: {}", reset.agent, why.detail),
      }
    })
  }
}

/// The rooms of those named whose record holds the companion as its one answerer.
async fn answered_alone<C: ConnectionTrait>(db: &C, agent: &str, named: &[i64]) -> Result<Vec<i64>, Fault> {
  if named.is_empty() {
    return Ok(vec![]);
  }

  let rows = all(
    db,
    &format!(
      r#"SELECT "chat_id" FROM "room_answerers" WHERE "chat_id" IN ({}) GROUP BY "chat_id"
         HAVING count(*) = 1 AND max("agent") = $1"#,
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

/// The receiver's rows of the ranges the reset names taken in one write, answered with what each table gave up.
pub async fn take(receiver: &Receiver, reset: &Reset) -> Result<Taken, Fault> {
  let named = |range: &str| reset.ranges.iter().any(|one| one == range);
  let transaction = receiver.db().begin().await.map_err(Fault::from)?;
  let mut taken: Taken = vec![];

  if named(CONVERSATION) {
    let alone = answered_alone(&transaction, &reset.agent, &reset.alone).await?;

    if !alone.is_empty() {
      let rooms: Vec<sea_orm::Value> = alone.iter().map(|room| (*room).into()).collect();

      for table in ["messages", "chat_reads"] {
        let sql = format!(r#"DELETE FROM "{table}" WHERE "chat_id" IN ({})"#, numbered(1, alone.len()));

        taken.push((table, run(&transaction, &sql, rooms.clone()).await?));
      }
    }

    for table in ["instructions", "room_windows"] {
      let sql = format!(r#"DELETE FROM "{table}" WHERE "agent" = $1"#);

      taken.push((table, run(&transaction, &sql, vec![reset.agent.clone().into()]).await?));
    }
  }

  if named(PERSON) {
    let sql = r#"DELETE FROM "people_reads" WHERE "agent" = $1"#;

    taken.push(("people_reads", run(&transaction, sql, vec![reset.agent.clone().into()]).await?));
  }

  transaction.commit().await.map_err(Fault::from)?;

  Ok(taken)
}

//! Pressure on the machine, as the agency republishes the node service's (docs/wake.md, 「What the agency's events
//! wake」): each `machine-pressure` told to the chat agent in the owner's own chat as a fact the system recorded,
//! naming the jobs behind the places that hold the memory, with an English account of it the console sums it up by.

use futures_util::future::BoxFuture;
use receiver_policy::fill_marks;
use receiver_policy::owner_facts::OWNER_FACTS;
use receiver_ports::agency::Event;
use serde_json::{Map, Value, json};

use crate::chat::notices::{Recorded, queue_fact};
use crate::chat::restart::kick;
use crate::hearing::Hearer;
use crate::receiver::Receiver;

/// The event a pressure the node service told is published as.
pub const PRESSURE: &str = "machine-pressure";

// The cause a pressure names that is the memory's.
const MEMORY: &str = "memory";

// What one field of the event reads as, where it is a number.
fn number(event: &Value, key: &str) -> Option<f64> {
  event.get(key).and_then(Value::as_f64)
}

// A number as the account says it: whole where it is whole, to one place otherwise.
fn shown(value: f64) -> String {
  let rounded = (value * 10.0).round() / 10.0;

  if rounded.fract() == 0.0 { format!("{rounded:.0}") } else { format!("{rounded:.1}") }
}

// The memory a holder holds as the account says it.
fn size(megabytes: f64) -> String {
  if megabytes >= 1024.0 {
    format!("{:.1}GB", megabytes / 1024.0)
  } else {
    format!("{megabytes:.0}MB")
  }
}

/// One holder as the account names it: the job at work in its place, with its item where it names one, or the
/// place alone; its memory; and the browsers it holds where it holds any.
pub fn holder_words(holder: &Value) -> String {
  let said = &OWNER_FACTS.machine;
  let text = |key: &str| holder.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
  let place = text("place");
  let who = match holder.get("jobId").and_then(Value::as_i64) {
    None => place,
    Some(job) => fill_marks(
      &said.job,
      &[
        ("job", &job.to_string()),
        (
          "todo",
          &holder
            .get("todoId")
            .and_then(Value::as_i64)
            .map_or_else(String::new, |todo| fill_marks(&said.todo, &[("todo", &todo.to_string())])),
        ),
        ("worker", &text("worker")),
        ("place", &place),
      ],
    ),
  };
  let browsers = holder.get("browsers").and_then(Value::as_i64).unwrap_or(0);
  let browsers = if browsers > 0 {
    fill_marks(&said.browsers, &[("browsers", &browsers.to_string())])
  } else {
    String::new()
  };

  fill_marks(
    &said.holder,
    &[
      ("who", &who),
      ("size", &size(number(holder, "megabytes").unwrap_or(0.0))),
      ("browsers", &browsers),
    ],
  )
}

/// The English account of a pressure: what stands over which line, and who holds the most of the memory.
pub fn account(event: &Value) -> String {
  let said = &OWNER_FACTS.machine;
  let top: Vec<String> = event
    .get("holders")
    .and_then(Value::as_array)
    .map(|holders| holders.iter().take(said.account_holders).map(holder_words).collect())
    .unwrap_or_default();
  let holders = if top.is_empty() {
    String::new()
  } else {
    fill_marks(&said.holders, &[("holders", &top.join(&said.joiner))])
  };
  let shown_of = |key: &str| number(event, key).map(shown).unwrap_or_default();

  if event.get("cause").and_then(Value::as_str) == Some(MEMORY) {
    fill_marks(
      &said.memory,
      &[
        ("full", &shown_of("fullPct")),
        ("minutes", &shown_of("minutes")),
        ("alert", &shown_of("alertPct")),
        ("holders", &holders),
      ],
    )
  } else {
    fill_marks(
      &said.disk,
      &[("free", &shown_of("freeGb")), ("limit", &shown_of("limitGb")), ("holders", &holders)],
    )
  }
}

/// The fact a pressure is told by: its cause and the figures of it, the node service's reading, the jobs open, the
/// largest holders and processes, and its account.
pub fn fact(event: &Value) -> Value {
  let said = &OWNER_FACTS.machine;
  let field = |key: &str| event.get(key).cloned().unwrap_or(Value::Null);
  let cause = event.get("cause").and_then(Value::as_str).unwrap_or_default();
  let mut fact = Map::new();

  fact.insert("state".into(), json!(PRESSURE));
  fact.insert("cause".into(), json!(cause));

  if cause == MEMORY {
    fact.insert("minutes".into(), field("minutes"));
    fact.insert("alertPct".into(), field("alertPct"));
    fact.insert("memoryFullPct".into(), field("fullPct"));
  } else {
    fact.insert("limitGb".into(), field("limitGb"));
    fact.insert("diskFreeGb".into(), field("freeGb"));
  }

  let holders: Vec<Value> = event
    .get("holders")
    .and_then(Value::as_array)
    .map(|holders| holders.iter().take(said.fact_holders).cloned().collect())
    .unwrap_or_default();

  fact.insert("reading".into(), field("reading"));
  fact.insert("jobs".into(), field("jobs"));
  fact.insert("browserJobs".into(), field("browserJobs"));
  fact.insert("holders".into(), Value::Array(holders));
  fact.insert("largest".into(), event.get("largest").cloned().unwrap_or_else(|| json!([])));
  fact.insert("account".into(), json!(account(event)));

  Value::Object(fact)
}

/// The pressure as the service hears it on the agency's stream.
pub struct Pressure {
  receiver: Receiver,
}

impl Pressure {
  pub fn new(receiver: Receiver) -> Self {
    Pressure { receiver }
  }
}

impl Hearer for Pressure {
  fn kinds(&self) -> &'static [&'static str] {
    &[PRESSURE]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      let said = &OWNER_FACTS.machine;
      let fact = fact(&event.data);
      let cause = fact["cause"].as_str().unwrap_or_default();
      let kind = format!("{}:{cause}", said.kind);
      let recorded = Recorded {
        fact: &fact,
        intent: &said.intent,
        kind: &kind,
        sort: &said.sort,
        summary: fact["account"].as_str().unwrap_or_default(),
      };

      match queue_fact(&self.receiver, self.receiver.owner_id(), &recorded).await {
        Ok(_) => kick(&self.receiver),
        Err(why) => eprintln!("bmo-receiver: the machine's pressure was not told: {why}"),
      }
    })
  }
}

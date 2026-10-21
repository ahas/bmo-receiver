//! The agency's events as the receiver hears them (docs/boundary-api.md, 「The agency」; docs/wake.md, 「A
//! companion's reset」 and 「What the agency's events wake」): each kind published on the stand-in session's stream
//! and handed to the part of the receiver that owns its effect, which takes a companion's rows, queues the owner's
//! facts, notices and orders, or says its line in the owner's chat.

use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::fallback::{self, Fallbacks};
use bmo_receiver::chat::instructions::{Queued, queue};
use bmo_receiver::chat::logins::{LOGIN_NOTICE, Logins};
use bmo_receiver::chat::overtime::{self, Overtime};
use bmo_receiver::companion::reset::{RESET, Reset, Resets, take};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::hearing::{Hearer, Hearing};
use bmo_receiver::receiver::wake::rest::{RESTING, Rests};
use bmo_receiver::store::settings;
use bmo_receiver::verbs::reload::{DOCUMENTS_MOVED, Places, Reloads};
use bmo_receiver::workers::{DOWNGRADED, Workers};
use bmo_receiver::{all, now_ms, run};
use sea_orm::DatabaseConnection;
use serde_json::json;

use crate::common::Scratch;
use crate::common::session::StandInSession;
use crate::common::units::{AGENT, OWNER, StandInChat, Standing, registration, standing};

/// The companion a reset is of.
const COMPANION: &str = "agent-c";

/// A room the companion answers alone.
const ALONE: i64 = -100;

/// A room the companion shares with the chat agent.
const SHARED: i64 = -200;

/// A room the event calls the companion's alone that the record says another agent answers too.
const CLAIMED: i64 = -300;

/// One instruction row as a test reads it back.
#[derive(Debug)]
struct Row {
  agent: String,
  chat_id: i64,
  text: String,
  kind: Option<String>,
  sort: Option<String>,
  expires_at: Option<i64>,
  rides: i64,
  ride_until: Option<i64>,
}

/// A receiver hearing the stand-in session's stream through every hearer the service stands with.
struct Heard {
  _scratch: Scratch,
  data: tempfile::TempDir,
  standing: Standing,
  chat: Arc<StandInChat>,
  session: Arc<StandInSession>,
}

impl Heard {
  async fn on() -> Self {
    let scratch = Scratch::new().await;
    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));
    let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()]);
    let session = Arc::new(StandInSession::new("agent-a@agent", "unused"));
    let mut agent = registration(AGENT, "Agent A", vec!["agent-a"], None, true);

    agent.row.insert("session_scope".to_owned(), json!("unified"));
    standing
      .agency
      .registering(vec![agent, registration(COMPANION, "Agent C", vec!["agent-c"], None, false)]);
    standing.agency.carrying_on(session.clone());

    let receiver = &standing.receiver;
    let places = Places {
      tree: Some(data.path().to_path_buf()),
      brain: data.path().join("brain"),
    };
    let hearers: Vec<Arc<dyn Hearer>> = vec![
      Arc::new(Resets::new(receiver.clone())),
      Arc::new(Reloads::new(receiver.clone(), places)),
      Arc::new(Fallbacks::new(receiver.clone())),
      Arc::new(Rests::new(receiver.clone())),
      Arc::new(Logins::new(receiver.clone())),
      Arc::new(Workers::new(receiver.clone())),
      Arc::new(Overtime::new(receiver.clone())),
    ];

    Hearing::new(hearers).keep(standing.agency.clone());
    until("the stream opened", || async { session.listeners() == 1 }).await;

    Heard {
      _scratch: scratch,
      data,
      standing,
      chat,
      session,
    }
  }

  fn db(&self) -> &DatabaseConnection {
    self.standing.receiver.db()
  }

  async fn sql(&self, statement: &str) {
    run(self.db(), statement, vec![]).await.unwrap();
  }

  async fn count(&self, statement: &str) -> i64 {
    let rows = all(self.db(), statement, vec![]).await.unwrap();

    rows.len() as i64
  }

  // The rooms a table holds rows of, in order.
  async fn rooms(&self, table: &str) -> Vec<i64> {
    all(self.db(), &format!(r#"SELECT "chat_id" FROM "{table}" ORDER BY "chat_id""#), vec![])
      .await
      .unwrap()
      .iter()
      .map(|row| row.try_get::<i64>("", "chat_id").unwrap())
      .collect()
  }

  // Every run record's run name, in order.
  async fn runs(&self) -> Vec<String> {
    all(self.db(), r#"SELECT "run" FROM "run_records" ORDER BY "run""#, vec![])
      .await
      .unwrap()
      .iter()
      .map(|row| row.try_get::<String>("", "run").unwrap())
      .collect()
  }

  async fn rows(&self) -> Vec<Row> {
    all(
      self.db(),
      r#"SELECT "agent", "chat_id", "text", "kind", "sort", "expires_at", "rides", "ride_until"
         FROM "instructions" ORDER BY "id""#,
      vec![],
    )
    .await
    .unwrap()
    .iter()
    .map(|row| Row {
      agent: row.try_get("", "agent").unwrap(),
      chat_id: row.try_get("", "chat_id").unwrap(),
      text: row.try_get("", "text").unwrap(),
      kind: row.try_get("", "kind").unwrap(),
      sort: row.try_get("", "sort").unwrap(),
      expires_at: row.try_get("", "expires_at").unwrap(),
      rides: row.try_get("", "rides").unwrap(),
      ride_until: row.try_get("", "ride_until").unwrap(),
    })
    .collect()
  }

  // The rows of a kind once as many as `count` stand.
  async fn kept(&self, kind: &str, count: usize) -> Vec<Row> {
    until(kind, || async { self.of_kind(kind).await.len() == count }).await;
    self.of_kind(kind).await
  }

  async fn of_kind(&self, kind: &str) -> Vec<Row> {
    self.rows().await.into_iter().filter(|row| row.kind.as_deref() == Some(kind)).collect()
  }

  // Every line said in the owner's chat, as the words it carried.
  fn said(&self) -> Vec<String> {
    self.chat.sent().iter().map(|send| serde_json::to_string(send).unwrap()).collect()
  }
}

// Waits, bounded, until `holds` does.
async fn until<F, T>(what: &str, holds: F)
where
  F: Fn() -> T,
  T: Future<Output = bool>,
{
  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while !holds().await {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "{what} never came");
}

#[tokio::test]
async fn a_companion_reset_takes_its_rows_of_the_ranges_named_and_the_record_of_its_rooms_alone() {
  let heard = Heard::on().await;

  heard.sql(&format!(r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ({ALONE}, '{COMPANION}'), ({SHARED}, '{COMPANION}'), ({SHARED}, '{AGENT}'), ({CLAIMED}, '{COMPANION}'), ({CLAIMED}, '{AGENT}')"#)).await;

  for room in [ALONE, SHARED, CLAIMED] {
    heard
      .sql(&format!(r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_name", "sent_at", "text") VALUES ({room}, 1, 'tester', 1, 'words')"#))
      .await;
    heard
      .sql(&format!(
        r#"INSERT INTO "chat_reads" ("chat_id", "last_read_message_id") VALUES ({room}, 1)"#
      ))
      .await;
  }

  for agent in [COMPANION, AGENT] {
    queue(heard.db(), agent, SHARED, "an instruction", &Queued::default()).await.unwrap();
    heard
      .sql(&format!(
        r#"INSERT INTO "room_windows" ("chat_id", "agent", "open_until") VALUES ({SHARED}, '{agent}', 1)"#
      ))
      .await;
    heard.sql(&format!(r#"INSERT INTO "people_reads" ("agent", "chat_id", "message_id", "read_at") VALUES ('{agent}', {SHARED}, 1, 1)"#)).await;
  }

  // A run of the room answered alone, one of the companion's own in the shared room, one of the chat agent's
  // there, one of the room the event calls alone that the record says another agent answers, and a job's.
  let companion_run = format!("chat:{SHARED}:2:1");

  for run in [
    format!("chat:{ALONE}:1:1"),
    companion_run.clone(),
    format!("chat:{SHARED}:3:1"),
    format!("chat:{CLAIMED}:4:1"),
    "bmo-task-1".to_owned(),
  ] {
    heard
      .sql(&format!(
        r#"INSERT INTO "run_records" ("run", "kind", "text", "at") VALUES ('{run}', 'note', 'a note', 1)"#
      ))
      .await;
  }

  heard.session.told(
    RESET,
    json!({
      "agent": COMPANION,
      "ranges": ["conversation", "person"],
      "alone": [ALONE, CLAIMED],
      "shared": [SHARED],
      "runs": [companion_run],
      "by": "tester",
      "at": 1,
    }),
  );
  until("the reset's rows taken", || async {
    heard
      .count(&format!(r#"SELECT 1 FROM "people_reads" WHERE "agent" = '{COMPANION}'"#))
      .await
      == 0
  })
  .await;

  assert_eq!(
    heard.rooms("messages").await,
    [CLAIMED, SHARED],
    "the room answered alone loses its record"
  );
  assert_eq!(heard.rooms("chat_reads").await, [CLAIMED, SHARED], "and its read mark");
  assert_eq!(
    heard.runs().await,
    ["bmo-task-1".to_owned(), format!("chat:{SHARED}:3:1"), format!("chat:{CLAIMED}:4:1"),],
    "the alone room's run records go by name, the companion's own run record by the name its log gave, and \
     the other agents' and the job's stay"
  );

  for table in ["instructions", "room_windows", "people_reads"] {
    let agents: Vec<String> = all(heard.db(), &format!(r#"SELECT "agent" FROM "{table}""#), vec![])
      .await
      .unwrap()
      .iter()
      .map(|row| row.try_get::<String>("", "agent").unwrap())
      .collect();

    assert_eq!(agents, [AGENT], "the companion's own {table} go and the chat agent's stay");
  }
}

#[tokio::test]
async fn a_dry_run_counts_the_run_records_a_reset_would_take_and_writes_nothing() {
  let heard = Heard::on().await;

  heard.sql(&format!(r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ({ALONE}, '{COMPANION}'), ({CLAIMED}, '{COMPANION}'), ({CLAIMED}, '{AGENT}')"#)).await;
  heard
    .sql(&format!(r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_name", "sent_at", "text") VALUES ({ALONE}, 1, 'tester', 1, 'words')"#))
    .await;
  heard
    .sql(&format!(
      r#"INSERT INTO "chat_reads" ("chat_id", "last_read_message_id") VALUES ({ALONE}, 1)"#
    ))
    .await;
  queue(heard.db(), COMPANION, SHARED, "an instruction", &Queued::default())
    .await
    .unwrap();
  heard
    .sql(&format!(
      r#"INSERT INTO "room_windows" ("chat_id", "agent", "open_until") VALUES ({SHARED}, '{COMPANION}', 1)"#
    ))
    .await;
  heard.sql(&format!(r#"INSERT INTO "people_reads" ("agent", "chat_id", "message_id", "read_at") VALUES ('{COMPANION}', {SHARED}, 1, 1)"#)).await;

  for run in [format!("chat:{ALONE}:1:1"), format!("chat:{SHARED}:2:1"), format!("chat:{SHARED}:3:1")] {
    heard
      .sql(&format!(
        r#"INSERT INTO "run_records" ("run", "kind", "text", "at") VALUES ('{run}', 'note', 'a note', 1)"#
      ))
      .await;
  }

  let reset = Reset {
    agent: COMPANION.to_owned(),
    ranges: vec!["conversation".to_owned(), "person".to_owned()],
    alone: vec![ALONE, CLAIMED],
    runs: vec![format!("chat:{SHARED}:2:1")],
  };
  let taken = take(&heard.standing.receiver, &reset, true).await.unwrap();
  let counted = |table: &str| taken.iter().find(|(name, _)| *name == table).map(|(_, rows)| *rows);

  assert_eq!(counted("messages"), Some(1), "the alone room's message would go");
  assert_eq!(counted("chat_reads"), Some(1), "and its read mark");
  assert_eq!(counted("run_records"), Some(2), "the alone room's run record and the companion's own");
  assert_eq!(counted("instructions"), Some(1), "the companion's own quiet instruction");
  assert_eq!(counted("room_windows"), Some(1), "and its window");
  assert_eq!(counted("people_reads"), Some(1), "and the mark its book read");
  assert_eq!(
    heard.runs().await,
    [format!("chat:{ALONE}:1:1"), format!("chat:{SHARED}:2:1"), format!("chat:{SHARED}:3:1"),],
    "the dry run leaves every run record where it stands"
  );
  assert_eq!(heard.count(r#"SELECT 1 FROM "messages""#).await, 1, "and every other row");
}

#[tokio::test]
async fn a_reset_of_the_person_range_alone_keeps_the_companions_conversation() {
  let heard = Heard::on().await;

  heard
    .sql(&format!(
      r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ({ALONE}, '{COMPANION}')"#
    ))
    .await;
  heard
    .sql(&format!(r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_name", "sent_at", "text") VALUES ({ALONE}, 1, 'tester', 1, 'words')"#))
    .await;
  heard.sql(&format!(r#"INSERT INTO "people_reads" ("agent", "chat_id", "message_id", "read_at") VALUES ('{COMPANION}', {ALONE}, 1, 1)"#)).await;
  queue(heard.db(), COMPANION, ALONE, "an instruction", &Queued::default()).await.unwrap();

  heard.session.told(
    RESET,
    json!({ "agent": COMPANION, "ranges": ["person"], "alone": [ALONE], "shared": [] }),
  );
  until("the marks taken", || async {
    heard.count(r#"SELECT 1 FROM "people_reads""#).await == 0
  })
  .await;

  assert_eq!(heard.count(r#"SELECT 1 FROM "messages""#).await, 1, "the room's record stays");
  assert_eq!(heard.rows().await.len(), 1, "the companion's instruction stays");
}

#[tokio::test]
async fn a_worker_fallen_to_a_lesser_model_is_voiced_to_the_owner() {
  let heard = Heard::on().await;

  heard.session.told(
    DOWNGRADED,
    json!({ "worker": "worker-a", "launched": "model-a", "answeredOn": ["model-b"], "dropped": true, "job": 9 }),
  );

  let rows = heard.kept("worker-downgrade:worker-a", 1).await;

  assert_eq!((rows[0].chat_id, rows[0].sort.as_deref()), (OWNER, Some("notice")));
  assert!(rows[0].text.contains("came off model-b instead of model-a"), "{}", rows[0].text);
  assert!(rows[0].text.contains("It happened on job #9."));
  assert!(rows[0].text.contains("That session has been dropped"));
  assert!(rows[0].expires_at.is_some(), "the notice lapses");
}

#[tokio::test]
async fn a_stand_in_answering_is_kept_and_told_and_its_end_clears_it() {
  let heard = Heard::on().await;

  heard.session.told(
    fallback::CHANGED,
    json!({ "ownerId": OWNER, "previous": "codex", "provider": "openrouter", "model": "model-b" }),
  );

  let taken = heard.kept("fallback", 1).await;

  assert!(taken[0].text.contains("codex could not answer this run"), "{}", taken[0].text);
  assert!(taken[0].text.contains("through openrouter on model-b"));
  assert_eq!(
    settings::get(heard.db(), fallback::PROVIDER_KEY).await.unwrap().as_deref(),
    Some("openrouter")
  );
  assert!(settings::get(heard.db(), fallback::SINCE_KEY).await.unwrap().is_some());

  heard
    .session
    .told(fallback::CHANGED, json!({ "ownerId": OWNER, "previous": "openrouter" }));
  heard.kept("fallback-lifted", 1).await;

  assert!(settings::get(heard.db(), fallback::SINCE_KEY).await.unwrap().is_none());
  assert!(settings::get(heard.db(), fallback::PROVIDER_KEY).await.unwrap().is_none());
}

#[tokio::test]
async fn the_deployment_resting_is_told_in_its_fixed_line_and_a_logins_end_in_the_agencys_words() {
  let heard = Heard::on().await;

  heard.session.told(
    RESTING,
    json!({ "account": "account-a", "kind": "limit", "until": 1_900_000_000, "notice": "resting" }),
  );
  heard.session.told(
    RESTING,
    json!({ "account": null, "kind": "outage", "until": 1_900_000_000, "notice": "down" }),
  );
  heard.session.told(LOGIN_NOTICE, json!({ "account": "account-a", "login": "here", "stands": "ending", "leftMs": 60_000, "notice": "The login ends soon: run the login command." }));
  until("three lines in the owner's chat", || async { heard.said().len() == 3 }).await;

  let said = heard.said();

  assert!(said[0].contains("rest-begins in en"), "{}", said[0]);
  assert!(said[1].contains("provider-down in en"), "{}", said[1]);
  assert!(said[2].contains("The login ends soon: run the login command."), "{}", said[2]);
  assert!(heard.rows().await.is_empty(), "neither wakes a session");
}

#[tokio::test]
async fn a_run_past_its_clock_is_an_order_withdrawn_where_its_ask_closes_before_a_round_took_it() {
  let heard = Heard::on().await;
  let asked = |ask: i64, run: &str| {
    json!({
      "ask": ask, "agent": "worker-a", "run": run, "what": "a job's turn", "startedAt": now_ms() - 600_000,
      "boundMs": 300_000, "lastActivityAt": null, "lastActivity": null, "answerBy": now_ms() + 1_800_000,
      "verb": "bmo-agency overtime",
    })
  };

  heard.session.told(overtime::ASKED, asked(5, "run-1"));
  heard.session.told(overtime::ASKED, asked(6, "run-2"));

  let order = heard.kept("overtime:run-1", 1).await;
  heard.kept("overtime:run-2", 1).await;

  assert_eq!((order[0].agent.as_str(), order[0].chat_id), (AGENT, OWNER));
  assert!(order[0].text.contains("Ask: #5"));
  assert!(order[0].text.contains("Clock: 5 minutes"));
  assert!(order[0].text.contains("Last activity: nothing recorded"));
  assert!(
    order[0].text.contains("`bmo-agency overtime --keep 5 [--for <length>]"),
    "{}",
    order[0].text
  );

  // The second order has been handed to a round, which a closing no longer withdraws.
  heard
    .sql(r#"UPDATE "instructions" SET "handed_at" = 1 WHERE "kind" = 'overtime:run-2'"#)
    .await;
  heard
    .session
    .told(overtime::CLOSED, json!({ "ask": 6, "run": "run-2", "verdict": "ended", "by": null }));
  heard
    .session
    .told(overtime::CLOSED, json!({ "ask": 5, "run": "run-1", "verdict": "keep", "by": "agent" }));
  heard.kept("overtime:run-1", 0).await;

  assert_eq!(heard.of_kind("overtime:run-2").await.len(), 1);
}

#[tokio::test]
async fn a_brain_reload_tells_the_named_agents_sessions_in_the_named_rooms_by_a_row_riding_for_good() {
  let heard = Heard::on().await;
  let document = format!("rooms/{ALONE}/people/someone.md");

  heard.sql(&format!(r#"INSERT INTO "bmo_rooms" ("chat_id") VALUES ({ALONE})"#)).await;
  heard.session.told(
    DOCUMENTS_MOVED,
    json!({ "paths": [document], "now": false, "agent": AGENT, "rooms": [ALONE, SHARED] }),
  );

  let rows = heard.kept(&format!("reload:{AGENT}"), 1).await;
  let whole = heard.data.path().join("brain").join(&document);

  assert_eq!(rows[0].chat_id, ALONE, "only a room holding the agent's session is told");
  assert_eq!((rows[0].rides, rows[0].ride_until), (2, None), "it rides any room's round, for good");
  assert!(
    rows[0].text.contains(&format!("[{}] (a reference document", whole.display())),
    "{}",
    rows[0].text
  );
}

#[tokio::test]
async fn a_reload_naming_no_rooms_carries_a_shipped_documents_text_read_from_the_document_tree() {
  let heard = Heard::on().await;
  let document = "prompts/agents/chat/procedures/one.md";
  let file = heard.data.path().join(document);

  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::fs::write(&file, "The procedure as it stands now.").unwrap();
  heard.sql(&format!(r#"INSERT INTO "bmo_rooms" ("chat_id") VALUES ({ALONE})"#)).await;
  heard
    .session
    .told(DOCUMENTS_MOVED, json!({ "paths": [document], "now": true, "agent": null, "rooms": [] }));

  let rows = heard.kept(&format!("reload:{AGENT}"), 1).await;

  assert_eq!(rows[0].chat_id, ALONE);
  assert!(
    rows[0].text.contains(&format!("[{document}]\nThe procedure as it stands now.")),
    "{}",
    rows[0].text
  );
}

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
use bmo_receiver::chat::recovered::{RECOVERED, Recoveries};
use bmo_receiver::companion::reset::{RESET, Resets};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::hearing::{Hearer, Hearing};
use bmo_receiver::machine::{PRESSURE, Pressure};
use bmo_receiver::receiver::wake::rest::{RESTING, Rests};
use bmo_receiver::store::settings;
use bmo_receiver::verbs::reload::{DOCUMENTS_MOVED, Places, Reloads};
use bmo_receiver::workers::{DOWNGRADED, LEFTOVERS, Workers};
use bmo_receiver::{all, now_ms, run};
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

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
  summary: Option<String>,
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
      Arc::new(Recoveries::new(receiver.clone())),
      Arc::new(Workers::new(receiver.clone())),
      Arc::new(Pressure::new(receiver.clone())),
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

  async fn rows(&self) -> Vec<Row> {
    all(
      self.db(),
      r#"SELECT "agent", "chat_id", "text", "kind", "sort", "summary", "expires_at", "rides", "ride_until"
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
      summary: row.try_get("", "summary").unwrap(),
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

  heard.session.told(
    RESET,
    json!({ "agent": COMPANION, "ranges": ["conversation", "person"], "alone": [ALONE, CLAIMED], "shared": [SHARED], "by": "tester", "at": 1 }),
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

// One holder of the machine's memory as the agency names it.
fn holder(place: usize, job: Option<i64>) -> Value {
  json!({
    "place": format!("place-{place}"),
    "megabytes": if place == 1 { 2047 } else { 1000 - place as i64 },
    "browsers": if place == 1 { 2 } else { 0 },
    "worker": job.map(|_| "worker-a"),
    "jobId": job,
    "todoId": job.map(|job| job + 100),
  })
}

#[tokio::test]
async fn pressure_on_the_machine_is_told_to_the_owners_session_as_a_fact_naming_the_jobs_behind_it() {
  let heard = Heard::on().await;
  let holders: Vec<Value> = (1..=10).map(|place| holder(place, (place == 1).then_some(12))).collect();
  let pressure = json!({
    "cause": "memory", "fullPct": 91.5, "alertPct": 85, "minutes": 3,
    "reading": { "memAvailableGb": 1.5 }, "holders": holders,
    "largest": [{ "pid": 7, "name": "browser", "megabytes": 900, "place": "place-1", "jobId": 12 }],
    "jobs": 4, "browserJobs": 1,
  });

  heard.session.told(PRESSURE, pressure.clone());

  let rows = heard.kept("machine-pressure:memory", 1).await;
  let row = &rows[0];
  let summary = row.summary.clone().unwrap();

  assert_eq!(
    (row.agent.as_str(), row.chat_id, row.sort.as_deref()),
    (AGENT, OWNER, Some("run_state"))
  );
  assert!(summary.starts_with(
    "The machine's memory pressure has stood at 91.5% for 3 minutes, over the alert line of 85%."
  ));
  assert!(
    summary.contains("job #12 (item #112) of worker-a in place-1 2.0GB, 2 browsers; place-2 998MB"),
    "{summary}"
  );
  assert!(!summary.contains("place-5"), "the account names the four largest holders");
  assert!(row.text.contains(r#""state":"machine-pressure""#), "{}", row.text);
  assert!(row.text.contains(r#""memoryFullPct":91.5"#));
  assert!(row.text.contains(r#""browserJobs":1"#));
  assert!(row.text.contains(r#""jobId":12"#));
  assert!(
    row.text.contains("place-8") && !row.text.contains("place-9"),
    "the fact carries the eight largest"
  );

  // A newer fact of the same cause replaces the one not yet handed over.
  heard.session.told(PRESSURE, pressure);
  heard
    .session
    .told(PRESSURE, json!({ "cause": "disk", "freeGb": 4.25, "limitGb": 10, "holders": [] }));

  let disk = heard.kept("machine-pressure:disk", 1).await;

  assert_eq!(heard.of_kind("machine-pressure:memory").await.len(), 1);
  assert!(
    disk[0]
      .summary
      .as_deref()
      .unwrap()
      .starts_with("The disk the data root stands on has 4.3GB free, under the 10GB line")
  );
  assert!(disk[0].text.contains(r#""diskFreeGb":4.25"#));
}

#[tokio::test]
async fn what_a_jobs_end_left_in_herdr_is_told_as_a_fact_summed_up_by_its_account() {
  let heard = Heard::on().await;

  heard.session.told(
    LEFTOVERS,
    json!({ "panes": 3, "workspaces": [{ "workspaceId": "w-1", "label": "job 9", "panes": 3, "marked": true }], "account": "Three panes stand in one workspace." }),
  );

  let rows = heard.kept("herdr-leftovers", 1).await;

  assert_eq!(rows[0].summary.as_deref(), Some("Three panes stand in one workspace."));
  assert!(rows[0].text.contains(r#""state":"herdr-leftovers""#));
  assert!(rows[0].text.contains(r#""workspaceId":"w-1""#));
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
async fn a_run_come_back_is_voiced_as_the_agency_composed_it() {
  let heard = Heard::on().await;

  heard.session.told(
    RECOVERED,
    json!({ "fact": "A run answered again.", "intent": "say the agent is working.", "kind": "doctor-recovered", "ttlMs": 3_600_000 }),
  );

  let rows = heard.kept("doctor-recovered", 1).await;

  assert_eq!(rows[0].chat_id, OWNER);
  assert!(rows[0].text.contains("A run answered again."));
  assert!(rows[0].expires_at.is_some_and(|at| at > now_ms() / 1000));
}

#[tokio::test]
async fn a_stand_in_answering_is_kept_and_told_and_its_end_clears_it() {
  let heard = Heard::on().await;

  heard.session.told(
    fallback::ANSWERING,
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
    .told(fallback::ENDED, json!({ "ownerId": OWNER, "previous": "openrouter" }));
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

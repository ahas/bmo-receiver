//! The tool host's catalogue as the receiver hears it (docs/boundary-api.md, 「The tool host」; docs/wake.md, 「The
//! tool host's changes」): a change becomes one riding fact for each supervisor whose standing tool session holds
//! one of its namespaces, a placing that replaced another version reads as updated, and a cursor the host no longer
//! holds is answered by hearing the stream again with none.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bmo_receiver::all;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::tools_heard::{concerned, hear, instruction};
use futures_util::future::BoxFuture;
use futures_util::{StreamExt, stream};
use receiver_policy::tool_host::TOOL_HOST;
use receiver_ports::http::{Answer, PortFailure, WireError};
use receiver_ports::tool_host::{Change, Entry, StandingSession, ToolEvent, ToolEvents, ToolHost};

use crate::integration::common::Scratch;
use crate::integration::common::units::{OWNER, registration, standing};

fn event(id: i64, kind: &str, tool: &str, namespaces: &[&str], replaced: bool) -> ToolEvent {
  ToolEvent {
    id,
    kind: kind.to_owned(),
    at: 1,
    data: Change {
      tool: tool.to_owned(),
      namespaces: namespaces.iter().map(|one| (*one).to_owned()).collect(),
      entries: vec![Entry {
        tool: format!("{}/{tool}", namespaces[0]),
        reached_by: "act".to_owned(),
        summary: "Does the one thing it is for.".to_owned(),
      }],
      replaced,
    },
  }
}

fn session(holder: &str, namespaces: &[&str]) -> StandingSession {
  StandingSession {
    holder_name: holder.to_owned(),
    namespaces: namespaces.iter().map(|one| (*one).to_owned()).collect(),
    created_at: 1,
  }
}

#[test]
fn a_change_reads_its_entries_and_a_replacing_placement_reads_as_updated() {
  let placed = instruction(&event(1, "tool.placed", "sample", &["bmo"], false));
  let updated = instruction(&event(2, "tool.placed", "sample", &["bmo"], true));
  let removed = instruction(&event(3, "tool.removed", "sample", &["bmo"], false));

  assert!(
    placed.contains("now serves sample")
      && placed.contains("bmo/sample (act) — Does the one thing it is for."),
    "{placed}"
  );
  assert!(updated.contains("an updated sample"), "{updated}");
  assert!(removed.contains("no longer serves sample"), "{removed}");

  for text in [&placed, &updated, &removed] {
    assert!(text.starts_with(&TOOL_HOST.marker), "the marker names the notice: {text}");
    assert!(!text.contains("bmo use <tool> --describe"), "the reading rule is in the block: {text}");
    assert!(!text.contains("Nothing needs saying in any room"), "the block carries the rule: {text}");
  }
}

#[test]
fn two_changes_in_one_turn_do_not_repeat_the_instruction() {
  let first = instruction(&event(1, "tool.placed", "sample", &["bmo"], false));
  let second = instruction(&event(2, "tool.removed", "other", &["web"], false));
  let turn = format!("{first}\n{second}");

  assert_eq!(turn.matches(&TOOL_HOST.marker).count(), 2, "each change carries its marker");
  assert!(turn.contains("now serves sample") && turn.contains("no longer serves other"), "{turn}");
  assert!(!turn.contains("bmo use <tool> --describe"), "the reading rule is not repeated: {turn}");
  assert!(
    !turn.contains("Nothing needs saying in any room"),
    "the block carries the rule once: {turn}"
  );
}

#[test]
fn a_change_concerns_each_supervisor_whose_standing_session_holds_one_of_its_namespaces() {
  let chat = registration("agent-a", "Agent A", vec![], None);
  let other = registration("agent-b", "Agent B", vec![], None);

  // A role's run holds a tool session too, and is told nothing: only the supervisors are listed.
  let agents = [chat, other];
  let sessions = [
    session("agent-a", &["bmo", "web"]),
    session("agent-b", &["gate"]),
    session("worker", &["web"]),
  ];

  assert_eq!(concerned(&agents, &sessions, &["web".to_owned()]), ["agent-a"]);
  assert_eq!(
    concerned(&agents, &sessions, &["gate".to_owned(), "bmo".to_owned()]),
    ["agent-a", "agent-b"]
  );
  assert!(concerned(&agents, &sessions, &["worker".to_owned()]).is_empty());
}

/// A tool host whose stream answers each opening as the test scripts it, keeping the cursor each opening named.
struct ScriptedHost {
  openings: Mutex<Vec<Answer<Vec<ToolEvent>>>>,
  cursors: Mutex<Vec<Option<i64>>>,
  sessions: Vec<StandingSession>,
}

impl ToolHost for ScriptedHost {
  fn events(&self, after: Option<i64>) -> BoxFuture<'_, Answer<ToolEvents>> {
    Box::pin(async move {
      self.cursors.lock().unwrap().push(after);

      let next = self.openings.lock().unwrap().remove(0);

      next.map(|events| stream::iter(events.into_iter().map(Ok)).boxed())
    })
  }

  fn sessions(&self) -> BoxFuture<'_, Answer<Vec<StandingSession>>> {
    Box::pin(async move { Ok(self.sessions.clone()) })
  }
}

#[tokio::test]
async fn each_event_is_one_riding_fact_per_agent_and_a_stale_cursor_hears_the_stream_again() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);
  let chat = registration("agent-a", "Agent A", vec![], None);
  let other = registration("agent-b", "Agent B", vec![], None);

  standing.agency.registering(vec![chat, other]);

  let gone = PortFailure::Answered {
    url: "http://tool-host/events".to_owned(),
    status: 410,
    answer: Some(WireError {
      code: "gone".to_owned(),
      error: "No event after 5 is held.".to_owned(),
    }),
  };
  let host = Arc::new(ScriptedHost {
    openings: Mutex::new(vec![
      Ok(vec![event(5, "tool.placed", "sample", &["bmo"], true)]),
      Err(gone),
      Ok(vec![event(9, "tool.removed", "other", &["web"], false)]),
    ]),
    cursors: Mutex::new(vec![]),
    sessions: vec![session("agent-a", &["bmo", "web"]), session("agent-b", &["web"])],
  });

  let ended = hear(&standing.receiver, host.as_ref(), Duration::ZERO, Some(3)).await;

  assert_eq!(
    *host.cursors.lock().unwrap(),
    [None, Some(5), None],
    "a stale cursor is dropped, not retried"
  );
  assert_eq!(ended, Some(9));

  let rows: Vec<(String, i64, String, Option<String>, i64)> = all(
    scratch.db(),
    r#"SELECT "agent", "chat_id", "text", "kind", "rides" FROM "instructions" ORDER BY "id""#,
    vec![],
  )
  .await
  .unwrap()
  .iter()
  .map(|row| {
    (
      row.try_get("", "agent").unwrap(),
      row.try_get("", "chat_id").unwrap(),
      row.try_get("", "text").unwrap(),
      row.try_get("", "kind").unwrap(),
      row.try_get("", "rides").unwrap(),
    )
  })
  .collect();
  let told: Vec<(&str, Option<&str>)> =
    rows.iter().map(|(agent, _, _, kind, _)| (agent.as_str(), kind.as_deref())).collect();

  assert_eq!(
    told,
    [
      ("agent-a", Some("tools-changed:agent-a:sample")),
      ("agent-a", Some("tools-changed:agent-a:other")),
      ("agent-b", Some("tools-changed:agent-b:other")),
    ]
  );
  assert!(rows[0].2.contains("an updated sample"), "{}", rows[0].2);
  assert!(rows[0].2.starts_with(&TOOL_HOST.marker), "the marker names the notice: {}", rows[0].2);
  assert!(
    rows.iter().all(|row| !row.2.contains("bmo use <tool> --describe")),
    "the reading rule stands in the block, not in a notice"
  );
  assert!(
    rows.iter().all(|row| row.1 == OWNER && row.4 == 2),
    "each rides any room's round of its session"
  );
}

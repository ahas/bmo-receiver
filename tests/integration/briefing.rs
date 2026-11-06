//! The voice briefing (docs/voiced-notices.md, 「The voice briefing」; docs/api.md, 「The voice briefing」): its two
//! switches moved one at a time and the whole one told to the owner, a line's full reading launched once and then
//! answered as it stands, its refusals, and a notice turn of the owner's chat summarised by its own session and
//! handed to the voice engine while a round somebody spoke in is not.

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::briefing::{line_opening, summary_of};
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use serde_json::json;
use tempfile::tempdir;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{get, post, put, stood};
use crate::integration::common::session::StandInSession;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const VOICE: &str = "voice-a";
const ROOM: i64 = -500;

fn voiced(voice: Option<&str>) -> receiver_ports::agency::Agent {
  let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);

  if let Some(voice) = voice {
    agent.row.insert("voice".to_owned(), json!(voice));
  }

  agent
}

#[tokio::test]
async fn the_switches_move_one_at_a_time_and_only_the_whole_ones_move_is_told_to_the_owner() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let (status, both) = get(&front, "/voice-briefing").await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(both, json!({ "on": true, "autoplay": true }), "a missing row reads as on");

  for refused in [json!({}), json!({ "on": false, "autoplay": false }), json!({ "on": "off" })] {
    let (status, body) = put(&front, "/voice-briefing", refused.clone()).await;

    assert_eq!(
      (status, body["code"].as_str()),
      (StatusCode::BAD_REQUEST, Some("one-switch")),
      "{refused}"
    );
  }

  let (_, both) = put(&front, "/voice-briefing", json!({ "on": false })).await;

  assert_eq!(both, json!({ "on": false, "autoplay": true }));

  put(&front, "/voice-briefing", json!({ "on": true })).await;
  put(&front, "/voice-briefing", json!({ "on": true })).await;

  let (_, both) = put(&front, "/voice-briefing", json!({ "autoplay": false })).await;

  assert_eq!(both, json!({ "on": true, "autoplay": false }));

  let told = scratch
    .texts(
      r#"SELECT "kind" || ' ' || "chat_id" FROM "instructions" WHERE "delivered_at" IS NULL ORDER BY "id""#,
    )
    .await;

  assert_eq!(
    told,
    [format!("voice-briefing {OWNER}")],
    "flipped twice, one line, and autoplay says nothing"
  );

  let text = scratch.texts(r#"SELECT "text" FROM "instructions""#).await;

  assert!(
    text[0].contains("switched the voice briefing on"),
    "the line is about where it ended: {}",
    text[0]
  );
}

#[tokio::test]
async fn a_briefing_line_goes_to_the_owners_private_room_even_where_a_notices_room_is_named() {
  const NOTICES: i64 = -9;

  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let mut given = deployment();

  given.notices_room = Some(NOTICES);

  let standing = unrun(scratch.db(), DataRoot::new(data.path()), vec![], given);

  standing.agency.registering(vec![voiced(Some(VOICE))]);
  bmo_receiver::chat::briefing::set(&standing.receiver, bmo_receiver::chat::briefing::SWITCH, false)
    .await
    .unwrap();

  assert_eq!(
    scratch
      .texts(r#"SELECT "kind" || ' ' || "chat_id" FROM "instructions" ORDER BY "id""#)
      .await,
    [format!("voice-briefing {OWNER}")],
    "the switch line stands in the owner's private room"
  );
}

async fn line(scratch: &Scratch, message_id: i64, agent: Option<&str>, sender: &str) {
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via", "agent_name")
         VALUES ($1, $2, 7, $4, 1760000000000, 'The backup finished. Nothing else changed.', 'road-a', $3)"#,
      vec![ROOM.into(), message_id.into(), agent.map(str::to_owned).into(), sender.into()],
    )
    .await;
}

#[tokio::test]
async fn a_lines_full_reading_is_launched_once_then_answered_as_it_stands_and_refused_where_it_cannot_be() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let reading = |message: i64| format!("/rooms/{ROOM}/messages/{message}/reading");

  front.standing.agency.registering(vec![voiced(Some(VOICE))]);
  line(&scratch, 11, Some(AGENT), "Agent A").await;
  line(&scratch, 12, None, "member").await;

  let (status, body) = post(&front, &reading(11), json!({})).await;

  assert_eq!(status, StatusCode::OK, "{body}");
  assert_eq!(body, json!({ "state": "making", "briefingId": null }), "the script is written first");

  until("the reading taken", || !front.standing.voice.briefed().is_empty()).await;

  let briefed = front.standing.voice.briefed();

  assert_eq!(briefed.len(), 1);
  assert_eq!(
    (
      briefed[0].kind.as_str(),
      briefed[0].voice_name.as_str(),
      briefed[0].message_id,
      briefed[0].plays
    ),
    ("full", VOICE, Some(11), false)
  );
  assert_eq!(
    briefed[0].text, "The backup finished. Nothing else changed.",
    "no script written: the line as it stands"
  );

  let (_, again) = post(&front, &reading(11), json!({})).await;

  assert_eq!(again, json!({ "state": "making", "briefingId": 1 }));
  *front.standing.voice.state.lock().unwrap() = "done".to_owned();

  let (_, ready) = post(&front, &reading(11), json!({})).await;

  assert_eq!(ready, json!({ "state": "ready", "briefingId": 1 }));
  assert_eq!(front.standing.voice.briefed().len(), 1, "a reading standing is not made again");

  let (status, _) = post(&front, &reading(12), json!({})).await;

  assert_eq!(status, StatusCode::NOT_FOUND, "a line nobody's agent sent has no reading");

  front.standing.agency.registering(vec![voiced(None)]);

  let (status, body) = post(&front, &reading(11), json!({})).await;

  assert_eq!((status, body["code"].as_str()), (StatusCode::CONFLICT, Some("voiceless")));

  put(&front, "/voice-briefing", json!({ "on": false })).await;

  let (status, body) = post(&front, &reading(11), json!({})).await;

  assert_eq!((status, body["code"].as_str()), (StatusCode::CONFLICT, Some("switched-off")));
}

async fn until(what: &str, holds: impl Fn() -> bool) {
  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while !holds() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "{what} never came");
}

#[test]
fn a_summary_answer_reads_as_its_title_and_words_and_a_line_opens_on_its_first_sentences() {
  assert_eq!(
    summary_of("The backup\nIt finished, and nothing else changed."),
    Some((Some("The backup".to_owned()), "It finished, and nothing else changed.".to_owned()))
  );
  assert_eq!(summary_of("Only words."), Some((None, "Only words.".to_owned())));
  assert_eq!(summary_of("<<SKIP>>"), None);
  assert_eq!(summary_of("  "), None);
  assert_eq!(line_opening("Done! More below.\nAnd more."), "Done! More below.");
  assert_eq!(
    line_opening("The nightly backup of every disk on the machine finished. Nothing else changed."),
    "The nightly backup of every disk on the machine finished."
  );
}

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(7),
    private_to: None,
  }
}

struct Owner {
  scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  session: Arc<StandInSession>,
  runner: Runner,
}

impl Owner {
  async fn new(session: StandInSession, voice: Option<&str>) -> Self {
    let scratch = Scratch::new().await;

    crate::integration::common::session::no_drafting(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));
    let prompts = data.path().join("prompts").join("chat");

    std::fs::create_dir_all(&prompts).unwrap();
    std::fs::write(prompts.join(&CHAT_LINE.room.owner_document), "The owner's own chat.").unwrap();
    std::fs::write(prompts.join(&CHAT_LINE.room.group_document), "A group room.").unwrap();
    std::fs::write(prompts.join(&CHAT_LINE.capabilities.document), "What the agent can do.").unwrap();

    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat], deployment()));
    let session = Arc::new(session);
    let receiver = standing.receiver.clone();
    let timings = Timings {
      queue: Duration::from_millis(1),
      wait: Duration::from_secs(60),
      window: Some(Duration::from_secs(60)),
      look: Duration::from_millis(5),
    };
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings, AGENT);

    standing.agency.registering(vec![voiced(voice)]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    Owner {
      scratch,
      _data: data,
      standing,
      session,
      runner,
    }
  }

  async fn over(&self, turns: usize) {
    let waited = tokio::time::timeout(Duration::from_secs(20), async {
      while self.session.prompts().len() < turns || !self.runner.idle() || self.session.working() {
        tokio::time::sleep(Duration::from_millis(5)).await;
      }
    })
    .await;

    assert!(waited.is_ok(), "the round never ended");
    tokio::time::sleep(Duration::from_millis(100)).await;
  }
}

#[tokio::test]
async fn a_notice_turn_of_the_owners_chat_is_summarised_by_its_own_session_and_handed_to_the_voice() {
  let owner = Owner::new(
    StandInSession::new("agent-a@agent", "unused")
      .answering(&["The nightly backup finished.", "Backup done\nThe backup finished overnight."]),
    Some(VOICE),
  )
  .await;

  owner
    .runner
    .of(AGENT)
    .instruct(OWNER, "The nightly backup finished.".to_owned(), None, None, vec![])
    .await;
  owner.over(2).await;

  let prompts = owner.session.prompts();

  assert_eq!(prompts.len(), 2, "{prompts:#?}");
  assert!(
    prompts[0].contains(&fill_marks(
      &CHAT_ROUND.voice.summary_rule,
      &[("marker", &CHAT_ROUND.voice.summary_marker)]
    )),
    "the room's block says what the marker asks for: {}",
    prompts[0]
  );
  assert!(
    prompts[1].contains(&CHAT_ROUND.voice.summary_marker)
      && !prompts[1].contains(&CHAT_ROUND.voice.summary_rule),
    "the summary turn carries the marker and its line, no instruction: {}",
    prompts[1]
  );
  assert!(prompts[1].contains("The nightly backup finished."));

  let briefed = owner.standing.voice.briefed();

  assert_eq!(briefed.len(), 1);
  assert_eq!(briefed[0].kind, "summary");
  assert_eq!(briefed[0].chat_id, OWNER);
  assert_eq!(briefed[0].title.as_deref(), Some("Backup done"));
  assert_eq!(briefed[0].text, "The backup finished overnight.");
  assert!(briefed[0].message_id.is_some() && briefed[0].plays);

  let kept = owner.scratch.texts(r#"SELECT "text" FROM "messages" ORDER BY "message_id""#).await;

  assert!(
    kept.iter().any(|line| line.contains("The nightly backup finished.")),
    "the line went out: {kept:?}"
  );
  assert!(
    kept.iter().all(|line| !line.contains("Backup done")),
    "the summary goes to no room: {kept:?}"
  );
}

#[tokio::test]
async fn a_round_the_owner_spoke_in_and_an_agent_with_no_voice_are_never_summarised() {
  let owner = Owner::new(StandInSession::new("agent-a@agent", "Here."), Some(VOICE)).await;
  let message = Inbound {
    chat_id: OWNER,
    message_id: 6,
    sender_id: Some(OWNER),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000_100,
    text: "what is new".into(),
    image_path: None,
    image_state: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: Some("bot-account".into()),
    via_reader: None,
    album: None,
  };

  owner
    .standing
    .receiver
    .message(&message, &Context::default(), owners_room())
    .await
    .unwrap();
  owner.over(1).await;

  assert_eq!(owner.session.prompts().len(), 1, "a conversation is not summarised");
  assert!(owner.standing.voice.briefed().is_empty());

  let voiceless = Owner::new(StandInSession::new("agent-a@agent", "The backup finished."), None).await;

  voiceless
    .runner
    .of(AGENT)
    .instruct(OWNER, "The backup finished.".to_owned(), None, None, vec![])
    .await;
  voiceless.over(1).await;

  assert_eq!(voiceless.session.prompts().len(), 1, "an agent with no voice is never spoken");
  assert!(voiceless.standing.voice.briefed().is_empty());
}

#[tokio::test]
async fn a_lines_full_reading_is_the_script_its_agents_session_writes_sent_as_written() {
  const SCRIPT: &str = "The backup finished overnight, and nothing else changed.";

  let owner = Owner::new(StandInSession::new("agent-a@agent", SCRIPT), Some(VOICE)).await;

  owner
    .scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via", "agent_name")
         VALUES ($1, 21, 7, 'Agent A', 1760000000000, 'Backup: done (2/2 disks, 0 errors).', 'road-a', $2)"#,
      vec![OWNER.into(), AGENT.into()],
    )
    .await;

  let first = bmo_receiver::chat::briefing::reading(&owner.standing.receiver, OWNER, 21).await;

  assert!(matches!(first, Ok(ref reading) if reading.briefing.is_none()), "{first:?}");

  until("the reading taken", || !owner.standing.voice.briefed().is_empty()).await;

  let prompts = owner.session.prompts();

  assert!(
    prompts[0].contains("read out in full") && prompts[0].contains("Backup: done (2/2 disks, 0 errors)."),
    "{}",
    prompts[0]
  );
  assert_eq!(
    owner.standing.voice.briefed()[0].text,
    SCRIPT,
    "the script goes to the engine exactly as written"
  );
}

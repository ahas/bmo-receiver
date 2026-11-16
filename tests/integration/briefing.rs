//! The voice briefing (docs/voiced-notices.md, 「The voice briefing」; docs/api.md, 「The voice briefing」): its two
//! switches moved one at a time and the whole one told to the owner, every briefing the engine holds stopped once the
//! whole one is off and none of them played after, a line's full reading launched once and then answered as it
//! stands, its refusals, and a notice turn of the owner's chat summarised by its own session and
//! handed to the voice engine while a round somebody spoke in is not, and no send of the agent's into the room
//! carried while its briefing turn is.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::StatusCode;
use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::briefing::{Summary, line_opening, reading_of, summary_of};
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use bmo_receiver::routes::routes;
use receiver_codes::SendRefusal;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use room_calls::sends::{Act, Asked, Asker, SayInput, SpeakInput};
use serde_json::{Value, json};
use tempfile::tempdir;
use tower::ServiceExt;

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

  let text = scratch
    .texts(r#"SELECT "text" FROM "instructions" WHERE "let_go" IS NULL ORDER BY "id""#)
    .await;

  assert!(
    text[0].contains("switched the voice briefing on"),
    "the line is about where it ended: {}",
    text[0]
  );
}

#[tokio::test]
async fn a_stop_the_engine_did_not_take_is_asked_again_and_every_write_of_off_asks_one() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let voice = front.standing.voice.clone();

  *voice.failing_stops.lock().unwrap() = 2;

  let (status, both) = put(&front, "/voice-briefing", json!({ "on": false })).await;

  assert_eq!((status, both["on"].as_bool()), (StatusCode::OK, Some(false)));
  until("the stop taken", || voice.stops().last() == Some(&true)).await;
  assert_eq!(voice.stops(), [false, false, true], "a failed stop is asked again until it is taken");

  put(&front, "/voice-briefing", json!({ "on": false })).await;
  until("a write of off where it stood stopping", || voice.stops().len() == 4).await;
}

#[tokio::test]
async fn switching_the_whole_briefing_off_stops_a_waiting_a_speaking_and_a_just_taken_briefing_and_none_plays_after()
 {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let voice = front.standing.voice.clone();
  let reading = |message: i64| format!("/rooms/{ROOM}/messages/{message}/reading");

  front.standing.agency.registering(vec![voiced(Some(VOICE))]);

  for message in [11, 12, 13] {
    line(&scratch, message, Some(AGENT), "Agent A").await;
  }

  post(&front, &reading(11), json!({})).await;
  until("the waiting reading taken", || voice.briefed().len() == 1).await;
  post(&front, &reading(12), json!({})).await;
  until("the speaking reading taken", || voice.briefed().len() == 2).await;
  voice.states.lock().unwrap().insert(2, "speaking".to_owned());

  // The third reading is handed over while the switch stands on, and the engine takes it only once it is off.
  let held = voice.taking.write().await;

  post(&front, &reading(13), json!({})).await;
  until("the third reading handed over", || voice.handed.load(Ordering::SeqCst) == 3).await;

  let (status, _) = put(&front, "/voice-briefing", json!({ "on": false })).await;

  assert_eq!(status, StatusCode::OK);
  until("the stop taken", || voice.stops() == [true]).await;
  assert_eq!(
    (voice.state_of(1), voice.state_of(2)),
    ("stopped".to_owned(), "stopped".to_owned()),
    "the waiting and the speaking briefing are stopped"
  );

  drop(held);
  until("the briefing taken as the switch moved stopped", || voice.state_of(3) == "stopped").await;
  assert_eq!(voice.stops(), [true, true], "taken after the off, it was stopped by a stop of its own");

  let (_, refused) = post(&front, &reading(11), json!({})).await;

  assert_eq!(refused["code"].as_str(), Some("switched-off"), "nothing is read while it is off");

  put(&front, "/voice-briefing", json!({ "on": true })).await;

  assert_eq!(
    (voice.handed.load(Ordering::SeqCst), voice.stops().len()),
    (3, 2),
    "switching it on hands the engine nothing"
  );
  assert!(
    (1..=3).all(|id| voice.state_of(id) == "stopped"),
    "no stopped briefing is ever taken up again"
  );

  let (_, asked) = post(&front, &reading(11), json!({})).await;

  assert_eq!(
    asked,
    json!({ "state": "making", "briefingId": null }),
    "a stopped reading is never answered"
  );
  until("a reading asked again launched anew", || voice.briefed().len() == 4).await;
  assert_eq!((voice.state_of(1), voice.state_of(4)), ("stopped".to_owned(), "waiting".to_owned()));
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
      .texts(
        r#"SELECT "kind" || ' ' || "chat_id" FROM "instructions" WHERE "let_go" IS NULL ORDER BY "id""#
      )
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

// The emotion line an answer opens on, naming `emotion`.
fn emotion_line(emotion: &str) -> String {
  format!("{} {emotion}", CHAT_ROUND.voice.emotion_label)
}

fn summary(emotion: Option<&str>, title: Option<&str>, text: &str) -> Option<Summary> {
  Some(Summary {
    emotion: emotion.map(str::to_owned),
    title: title.map(str::to_owned),
    text: text.to_owned(),
  })
}

#[test]
fn a_summary_answer_reads_as_its_emotion_title_and_words_and_a_line_opens_on_its_first_sentences() {
  let glad = &CHAT_ROUND.voice.emotions[1];

  assert_eq!(
    summary_of(&format!("{}\nThe backup\nIt finished, and nothing else changed.", emotion_line(glad))),
    summary(Some(glad), Some("The backup"), "It finished, and nothing else changed.")
  );
  assert_eq!(
    summary_of(&format!("{}\nThe backup\nIt finished.", emotion_line("cheerful"))),
    summary(None, Some("The backup"), "It finished."),
    "an emotion outside the set leaves the briefing without one, its line still taken off"
  );
  assert_eq!(
    summary_of("The backup\nIt finished, and nothing else changed."),
    summary(None, Some("The backup"), "It finished, and nothing else changed.")
  );
  assert_eq!(summary_of("Only words."), summary(None, None, "Only words."));
  assert_eq!(summary_of("<<SKIP>>"), None);
  assert_eq!(summary_of(&format!("{}\n<<SKIP>>", emotion_line(glad))), None);
  assert_eq!(summary_of("  "), None);
  assert_eq!(
    reading_of(Some(&format!("{}\nAll of it, read out.", emotion_line(glad))), "The line.".to_owned()),
    (Some(glad.clone()), "All of it, read out.".to_owned())
  );
  assert_eq!(
    reading_of(Some(&emotion_line(glad)), "The line.".to_owned()),
    (None, "The line.".to_owned()),
    "a script of an emotion line alone has the line spoken as it stands"
  );
  assert_eq!(reading_of(None, "The line.".to_owned()), (None, "The line.".to_owned()));
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
    sealed: false,
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
    StandInSession::new("agent-a@agent", "unused").answering(&[
      "The nightly backup finished.",
      &format!(
        "{}\nBackup done\nThe backup finished overnight.",
        emotion_line(&CHAT_ROUND.voice.emotions[1])
      ),
    ]),
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
    prompts[0].contains(&CHAT_ROUND.voice.summary_rule_stated())
      && CHAT_ROUND.voice.emotions.iter().all(|emotion| prompts[0].contains(emotion.as_str())),
    "the room's block says what the marker asks for, every emotion listed: {}",
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
  assert_eq!(briefed[0].emotion.as_ref(), Some(&CHAT_ROUND.voice.emotions[1]));
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

  let worried = &CHAT_ROUND.voice.emotions[7];
  let answer = format!("{}\n{SCRIPT}", emotion_line(worried));
  let owner = Owner::new(StandInSession::new("agent-a@agent", &answer), Some(VOICE)).await;

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
  let briefed = owner.standing.voice.briefed();

  assert_eq!(
    briefed[0].text, SCRIPT,
    "the script goes to the engine exactly as written, past its emotion line"
  );
  assert_eq!(briefed[0].emotion.as_ref(), Some(worried));
}

const TOKEN: &str = "the-token";

// A send the agent's session asks into `room` from a turn of its in the owner's chat, as its tools ask it, and
// what the receiver answered.
async fn sent(owner: &Owner, room: i64, act: Act) -> Value {
  let asker = Asker {
    room: Some(room),
    via: Some("road-a".to_owned()),
    agent_name: Some(AGENT.to_owned()),
    by: Some(format!("chat:{OWNER}")),
    ..Asker::default()
  };
  let request = Request::post("/sends")
    .header("authorization", format!("Bearer {TOKEN}"))
    .body(Body::from(serde_json::to_string(&Asked { asker, act }).unwrap()))
    .unwrap();
  let answer = routes(owner.standing.receiver.clone(), TOKEN).oneshot(request).await.unwrap();

  assert_eq!(answer.status(), StatusCode::OK);
  serde_json::from_slice(&to_bytes(answer.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn say(text: &str) -> Act {
  Act::Say(SayInput {
    chat: None,
    via: None,
    text: text.to_owned(),
    reply: false,
    reply_to: None,
  })
}

fn speak(text: &str) -> Act {
  Act::Speak(SpeakInput {
    chat: None,
    via: None,
    voice: VOICE.to_owned(),
    text: text.to_owned(),
    spoken: None,
    reply_to: None,
  })
}

fn briefing_refusal(room: i64) -> String {
  SendRefusal::BriefingTurn {
    agent: AGENT.to_owned(),
    room,
  }
  .to_string()
}

#[tokio::test]
async fn a_send_the_agent_asks_into_the_room_during_its_summary_turn_is_refused_and_no_other_is() {
  const LINE: &str = "The nightly backup finished.";

  let owner = Owner::new(
    StandInSession::new("agent-a@agent", "unused")
      .answering(&[LINE, "Backup done\nThe backup finished overnight."])
      .holding_turn(2),
    Some(VOICE),
  )
  .await;

  owner.runner.of(AGENT).instruct(OWNER, LINE.to_owned(), None, None, vec![]).await;
  until("the summary turn working", || {
    owner.session.prompts().len() == 2 && owner.session.working()
  })
  .await;
  assert!(owner.session.prompts()[1].contains(&CHAT_ROUND.voice.summary_marker));

  for act in [say(LINE), speak(LINE)] {
    let answer = sent(&owner, OWNER, act).await;

    assert_eq!(answer["refusal"], json!(briefing_refusal(OWNER)), "{answer}");
    assert_eq!(answer["lines"], json!([]));
    assert!(answer.get("handed").is_none() && answer.get("own").is_none(), "{answer}");
  }

  let elsewhere = sent(&owner, ROOM, say("Elsewhere.")).await;

  assert_ne!(
    elsewhere["refusal"],
    json!(briefing_refusal(ROOM)),
    "another room's send is judged as every send is: {elsewhere}"
  );
  assert_ne!(elsewhere["refusal"], json!(briefing_refusal(OWNER)));

  owner.session.let_go();
  owner.over(2).await;

  let after = sent(&owner, OWNER, say("Later.")).await;

  assert!(
    after.get("refusal").is_none() && after["handed"].is_i64(),
    "once the turn ended the send is handed over as any send outside a round: {after}"
  );
  assert_eq!(owner.standing.voice.briefed().len(), 1);

  let kept = owner.scratch.texts(r#"SELECT "text" FROM "messages" ORDER BY "message_id""#).await;

  assert_eq!(
    kept.iter().filter(|text| text.contains(LINE)).count(),
    1,
    "the line stands once, with no second message of it: {kept:?}"
  );
}

#[tokio::test]
async fn a_send_the_agent_asks_into_the_lines_room_during_its_reading_turn_is_refused_until_it_ends() {
  let owner = Owner::new(
    StandInSession::new("agent-a@agent", "The backup finished overnight.").holding_the_first(),
    Some(VOICE),
  )
  .await;

  owner
    .scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via", "agent_name")
         VALUES ($1, 31, 7, 'Agent A', 1760000000000, 'Backup: done.', 'road-a', $2)"#,
      vec![OWNER.into(), AGENT.into()],
    )
    .await;
  // A line of its own, since a reading being written is held by its room and line across the process.
  bmo_receiver::chat::briefing::reading(&owner.standing.receiver, OWNER, 31)
    .await
    .unwrap();
  until("the reading turn working", || owner.session.working()).await;

  let answer = sent(&owner, OWNER, speak("Backup: done.")).await;

  assert_eq!(answer["refusal"], json!(briefing_refusal(OWNER)), "{answer}");

  owner.session.let_go();
  until("the reading taken", || !owner.standing.voice.briefed().is_empty()).await;

  let after = sent(&owner, OWNER, say("Later.")).await;

  assert!(after.get("refusal").is_none() && after["handed"].is_i64(), "{after}");
}

//! Replacing the default answerer's one session (docs/unified-bmo.md, 「Replacing the session」): a turn on an unmoved
//! head resumes the session whatever its own composition carries, and a head the agency says moved leaves a
//! replacement owed, made once the round that found it has ended and no words wait: a replacement never stands
//! between a question and its answer, and a question arriving while it is made waits for the fresh session. Before
//! it is ended the old session writes its handoff file, which the new session's first turn carries; where it writes
//! nothing the receiver's transcript of it stands in, and only where neither stands is nothing recovered and the
//! owner told so. A session that cannot take the turn — the owner's word on a model replaced on its own, a full
//! window — is replaced ahead of it.

use std::sync::Arc;
use std::time::Duration;

use bmo_chat_client::ChatSend;
use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_policy::handover::HANDOVER;

use crate::integration::common::Scratch;
use crate::integration::common::session::StandInSession;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const ANSWER: &str = "Here is what was asked.";

/// The id the agent's one session goes by, the agent's name alone.
const SESSION: &str = AGENT;

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(7),
    private_to: None,
    sealed: false,
  }
}

fn said(message_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id: OWNER,
    message_id,
    sender_id: Some(OWNER),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000 + message_id,
    text: text.into(),
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
  }
}

fn timings() -> Timings {
  Timings {
    queue: Duration::from_millis(1),
    wait: Duration::from_secs(60),
    window: Some(Duration::from_secs(60)),
    look: Duration::from_millis(5),
  }
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

/// The default answerer on its one session across its rooms, in the owner's chat.
struct Unified {
  scratch: Scratch,
  data: tempfile::TempDir,
  standing: Standing,
  session: Arc<StandInSession>,
  runner: Runner,
  chat: Arc<StandInChat>,
}

impl Unified {
  async fn on(session: StandInSession) -> Self {
    let scratch = Scratch::new().await;

    crate::integration::common::session::no_drafting(scratch.db()).await;
    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

    crate::integration::common::session::prompt_documents(data.path());

    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
    let session = Arc::new(session);
    let receiver = standing.receiver.clone();
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
    let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);

    agent.row.insert("sessionScope".to_owned(), serde_json::json!("agent"));
    standing.agency.registering(vec![agent]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    Unified {
      scratch,
      data,
      standing,
      session,
      runner,
      chat,
    }
  }

  // The fixed lines the owner's chat was sent, as the stand-in web renders them: the code and the language.
  fn fixed_lines_said(&self) -> Vec<String> {
    self
      .chat
      .sent()
      .iter()
      .filter_map(|held| match &held.send {
        ChatSend::Text { text, .. } if text.ends_with(" in en") => Some(text.clone()),
        _ => None,
      })
      .collect()
  }

  // The notices the owner's chat was queued of a handoff lost.
  async fn lost_notices(&self) -> Vec<String> {
    self
      .scratch
      .texts(&format!(
        r#"SELECT "text" FROM "instructions" WHERE "chat_id" = {OWNER} AND "kind" = 'handoff-lost'"#
      ))
      .await
  }

  // The owner's words, and the round they open run to its end.
  async fn round(&self, message_id: i64, text: &str) {
    let before = self.session.prompts().len();

    self
      .standing
      .receiver
      .message(&said(message_id, text), &Context::default(), owners_room())
      .await
      .unwrap();
    until("the round's end", || {
      self.session.prompts().len() > before && self.runner.idle() && !self.session.working()
    })
    .await;
  }
}

#[tokio::test]
async fn two_rounds_of_a_room_on_an_unmoved_head_go_on_the_one_session() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.round(1, "the first words").await;
  unified.round(2, "the second words").await;

  let prompts = unified.session.prompts();

  // The two turns differ in their scope line and their words, and neither replaces the session.
  assert_eq!(prompts.len(), 2, "{prompts:?}");
  assert_ne!(prompts[0], prompts[1]);
  assert!(prompts[0].contains("TURN SCOPE") && prompts[1].contains("TURN SCOPE"));
  assert!(unified.standing.agency.ended().is_empty(), "no session was ended");

  let notes = [&HANDOVER.chat.handed_marker, &HANDOVER.chat.carried_marker, &HANDOVER.chat.lost_marker];

  assert!(
    prompts
      .iter()
      .all(|prompt| notes.iter().all(|marker| !prompt.starts_with(marker.as_str()))),
    "no turn opens on a replacement note: {prompts:?}"
  );
}

#[tokio::test]
async fn a_head_the_agency_says_moved_replaces_the_session_once_the_round_that_found_it_has_ended() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.round(1, "the first words").await;
  unified.session.prefix_changed("reload: a prompt document");
  unified.round(2, "the second words").await;

  let ended = unified.standing.agency.ended();

  assert_eq!(ended, [SESSION], "a reason the last turn did not keep replaces the session once");

  let prompts = unified.session.prompts();

  assert_eq!(prompts.len(), 3, "the round's turn, then the handoff asked after it: {prompts:?}");
  assert!(
    prompts[1].contains("the second words"),
    "the round's turn goes on in the session as it stands"
  );
  assert!(
    prompts[2].starts_with(&HANDOVER.chat.ask_marker),
    "the handoff is asked once the round ended"
  );

  let count = unified.session.prompts().len();

  unified.round(3, "the third words").await;
  assert_eq!(unified.standing.agency.ended(), ended, "the reason kept moves nothing more");
  assert!(unified.session.prompts().len() > count);
}

#[tokio::test]
async fn a_session_opened_before_with_no_record_of_its_head_is_replaced_and_one_never_opened_is_not() {
  let opened = Unified::on(StandInSession::new(SESSION, ANSWER).at_generation(3)).await;

  opened.round(1, "the first words").await;
  assert_eq!(opened.standing.agency.ended(), [SESSION]);

  let fresh = Unified::on(StandInSession::new(SESSION, ANSWER).at_generation(0)).await;

  fresh.round(1, "the first words").await;
  assert!(fresh.standing.agency.ended().is_empty());
}

const NOTE: &str = "In the middle of the owner's question about the deployment; promised a reply by noon.";

#[tokio::test]
async fn a_replacement_asks_the_old_session_for_its_handoff_file_and_the_new_session_opens_on_it() {
  let unified = Unified::on(
    StandInSession::new(SESSION, ANSWER).acting(crate::integration::common::session::writing_handoff(NOTE)),
  )
  .await;

  unified.round(1, "the first words").await;
  unified.session.prefix_changed("reload: a prompt document");
  unified.round(2, "the second words").await;

  let prompts = unified.session.prompts();

  assert_eq!(prompts.len(), 3, "{prompts:?}");

  // The old session was asked, once the round ended and before it was ended, to write its handoff at a file
  // under the data root, under the marker alone: the rule stands in the room's block.
  assert!(prompts[2].starts_with(&HANDOVER.chat.ask_marker), "{}", prompts[2]);
  assert!(
    !prompts[2].contains("First, consolidate your brain"),
    "no instruction sentence stands in the ask turn: {}",
    prompts[2]
  );
  let path =
    crate::integration::common::session::handoff_path(&prompts[2]).expect("the handoff names its file");
  let folder = unified.data.path().join("handoffs").join(AGENT);

  assert!(path.starts_with(&folder.display().to_string()), "{path}");
  assert_eq!(std::fs::read_to_string(&path).unwrap(), NOTE);
  // The handoff turn answers the owner's private chat, as every turn across rooms names a room.
  assert_eq!(unified.session.rooms()[2], Some(OWNER), "{:?}", unified.session.rooms());
  assert_eq!(
    std::fs::read_to_string(folder.join("latest.md")).unwrap(),
    NOTE,
    "the newest is copied beside it"
  );
  assert_eq!(unified.standing.agency.ended(), [SESSION]);

  // The new session's first turn opens on the file's path and its whole text, ahead of the round's own words.
  unified.round(3, "the third words").await;

  let prompts = unified.session.prompts();
  let first = &prompts[3];

  assert!(first.starts_with(&HANDOVER.chat.handed_marker), "{first}");
  assert!(first.contains(&path) && first.contains(NOTE), "{first}");
  assert!(first.find(NOTE) < first.find("the third words"), "{first}");
  assert!(!first.contains("could not be recovered"), "{first}");
  assert!(unified.lost_notices().await.is_empty());
  assert_eq!(unified.standing.agency.ended(), [SESSION], "the fresh session stands");
}

#[tokio::test]
async fn words_that_arrive_while_the_replacement_is_made_wait_for_the_fresh_session() {
  // The third turn, the handoff asked after the second round, stands working until it is let go.
  let unified = Unified::on(
    StandInSession::new(SESSION, ANSWER)
      .acting(crate::integration::common::session::writing_handoff(NOTE))
      .holding_turn(3),
  )
  .await;

  unified.round(1, "the first words").await;
  unified.session.prefix_changed("reload: a prompt document");
  unified
    .standing
    .receiver
    .message(&said(2, "the second words"), &Context::default(), owners_room())
    .await
    .unwrap();
  until("the handoff turn", || unified.session.prompts().len() == 3 && unified.session.working()).await;
  assert!(unified.session.prompts()[2].starts_with(&HANDOVER.chat.ask_marker));

  // Words arriving while the handoff is written go on no turn of the old session, and the room is told once,
  // however many arrive, that the memory is being tidied and the answer comes after.
  for (id, words) in [(3, "the third words"), (4, "the fourth words")] {
    unified
      .standing
      .receiver
      .message(&said(id, words), &Context::default(), owners_room())
      .await
      .unwrap();
  }

  tokio::time::sleep(Duration::from_millis(300)).await;
  assert_eq!(unified.session.prompts().len(), 3, "the words wait: {:?}", unified.session.prompts());
  assert!(
    unified.standing.agency.ended().is_empty(),
    "the old session still stands while it writes"
  );
  assert_eq!(unified.fixed_lines_said(), ["memory-tidying in en"], "the room is told once");

  unified.session.let_go();
  until("the third words' round", || {
    unified.session.prompts().len() == 4 && unified.runner.idle()
  })
  .await;

  let prompts = unified.session.prompts();

  assert!(prompts[3].starts_with(&HANDOVER.chat.handed_marker), "{}", prompts[3]);
  assert!(
    prompts[3].contains(NOTE)
      && prompts[3].contains("the third words")
      && prompts[3].contains("the fourth words"),
    "{}",
    prompts[3]
  );
  assert_eq!(unified.standing.agency.ended(), [SESSION]);
  assert_eq!(
    unified.fixed_lines_said(),
    ["memory-tidying in en"],
    "nothing more is said once the fresh session stands"
  );
}

#[tokio::test]
async fn a_session_that_writes_no_handoff_leaves_the_transcript_of_what_it_answered() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.round(1, "the first words").await;
  unified.session.prefix_changed("reload: a prompt document");
  unified.round(2, "the second words").await;
  assert_eq!(unified.standing.agency.ended(), [SESSION], "replaced once the second round ended");
  unified.round(3, "the third words").await;

  let prompts = unified.session.prompts();
  let first = prompts.last().unwrap();

  assert_eq!(prompts.len(), 4, "{prompts:?}");
  assert!(first.starts_with(&HANDOVER.chat.carried_marker), "{first}");
  assert!(first.contains("WHAT YOU ANSWERED BEFORE"), "{first}");
  assert!(first.contains(&format!("{AGENT} (room {OWNER}): {ANSWER}")), "{first}");
  assert!(!first.contains("could not be recovered"), "{first}");
  assert!(unified.lost_notices().await.is_empty());
}

#[tokio::test]
async fn a_session_the_agency_opened_without_its_handoff_opens_on_the_transcript_of_what_it_answered() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.round(1, "the first words").await;

  // The agency replaced the session and its ask for the handoff failed: its state says so until a turn answered.
  unified.session.handoff_lost(true);
  unified.round(2, "the second words").await;

  let prompts = unified.session.prompts();
  let first = prompts.last().unwrap();

  assert!(first.starts_with(&HANDOVER.chat.carried_marker), "{first}");
  assert!(first.contains(&format!("{AGENT} (room {OWNER}): {ANSWER}")), "{first}");
  assert!(unified.lost_notices().await.is_empty(), "a transcript was carried, so nothing is told");

  // The agency lets the word go once that turn answered; the next turn goes on as any other.
  unified.session.handoff_lost(false);
  unified.round(3, "the third words").await;

  let prompts = unified.session.prompts();

  assert!(!prompts.last().unwrap().starts_with(&HANDOVER.chat.carried_marker), "{prompts:?}");
}

#[tokio::test]
async fn a_session_the_agency_opened_without_its_handoff_and_no_transcript_opens_on_nothing_recovered_and_tells_the_owner()
 {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.session.handoff_lost(true);
  unified.round(1, "the first words").await;

  let prompts = unified.session.prompts();
  let first = prompts.last().unwrap();

  assert!(first.starts_with(&HANDOVER.chat.lost_marker), "{first}");

  let told = unified.lost_notices().await;

  assert_eq!(told.len(), 1, "{told:?}");
}

#[tokio::test]
async fn nothing_is_recovered_only_where_neither_a_handoff_nor_a_transcript_stands_and_the_owner_is_told() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  bmo_receiver::chat::cache::force_reset(
    unified.scratch.db(),
    AGENT,
    receiver_codes::SessionReason::DowngradeForced,
  )
  .await
  .unwrap();
  unified.round(1, "the first words").await;

  let prompts = unified.session.prompts();
  let first = prompts.last().unwrap();

  assert_eq!(prompts.len(), 2, "the handoff was asked, then the round: {prompts:?}");
  assert!(first.starts_with(&HANDOVER.chat.lost_marker), "{first}");
  assert!(first.contains(&HANDOVER.chat.lost), "{first}");

  let told = unified.lost_notices().await;

  assert_eq!(told.len(), 1, "{told:?}");
  assert!(told[0].contains("neither its handoff nor a transcript"), "{told:?}");

  // The owner's word to start afresh is spent by the one replacement.
  unified.round(2, "the second words").await;
  assert_eq!(unified.standing.agency.ended(), [SESSION]);
}

#[tokio::test]
async fn a_full_session_handed_over_by_its_note_is_replaced_at_its_next_turn_through_the_same_handoff() {
  let unified = Unified::on(
    StandInSession::new(SESSION, ANSWER).acting(crate::integration::common::session::writing_handoff(NOTE)),
  )
  .await;

  unified.round(1, "the first words").await;
  assert!(bmo_receiver::chat::fold::hand_off(&unified.standing.receiver, AGENT).await);
  unified.round(2, "the second words").await;

  let prompts = unified.session.prompts();

  assert_eq!(prompts.len(), 3, "{prompts:?}");
  assert!(
    crate::integration::common::session::handoff_path(&prompts[1]).is_some(),
    "{}",
    prompts[1]
  );
  assert!(prompts[2].contains(NOTE), "{}", prompts[2]);
  assert_eq!(unified.standing.agency.ended(), [SESSION]);
}

// The process row the supervisor's turns read, as the console sets it.
async fn process_row(unified: &Unified, rounds: &[&str], off: &[&str]) {
  use bmo_receiver::chat::draft::table::{ANY, PastCeiling, Process, set};

  let named = |list: &[&str]| list.iter().map(|name| (*name).to_owned()).collect();

  set(
    unified.scratch.db(),
    &Process {
      provider: ANY.to_owned(),
      tier: ANY.to_owned(),
      agent: AGENT.to_owned(),
      rounds: named(rounds),
      send_from: String::new(),
      off: named(off),
      stages_off: vec![],
      ceiling_seconds: 0,
      past_ceiling: PastCeiling::Send,
      set: true,
    },
  )
  .await
  .unwrap();
}

#[tokio::test]
async fn a_change_of_the_draft_process_instructions_replaces_the_session_once() {
  use bmo_receiver::chat::cache::{active_route, reset_reason};
  use receiver_codes::SessionReason;

  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.round(1, "the first words").await;
  assert!(unified.standing.agency.ended().is_empty());

  // A question asked that was not: the block's stage lines are no longer the ones the session read.
  process_row(&unified, &["final"], &[]).await;

  let db = unified.scratch.db();
  let route = active_route(db, AGENT).await.unwrap().expect("the first round kept its road");
  let state = unified.standing.receiver.units().agency.state(SESSION).await.unwrap_or_default();

  assert_eq!(
    reset_reason(db, AGENT, &route, &state).await.unwrap(),
    Some(SessionReason::Process),
    "the replacement names the process"
  );
  unified.round(2, "the second words").await;

  let ended = unified.standing.agency.ended();

  assert_eq!(ended, [SESSION], "the session that read the instructions as they were is replaced");
  assert!(
    unified.session.prompts()[1].contains("the second words"),
    "the round that found the change goes on in the session as it stood: {:?}",
    unified.session.prompts()
  );

  unified.round(3, "the third words").await;
  assert_eq!(unified.standing.agency.ended(), ended, "instructions that stand replace nothing more");
}

#[tokio::test]
async fn a_turn_that_judges_no_replacement_leaves_a_change_of_the_process_instructions_to_the_next_round() {
  use bmo_receiver::chat::cache::{Leaving, active_route, remember, reset_reason};
  use bmo_receiver::chat::room::process_mark;
  use receiver_codes::SessionReason;

  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.round(1, "the first words").await;
  process_row(&unified, &["final"], &[]).await;

  let db = unified.scratch.db();
  let route = active_route(db, AGENT).await.unwrap().expect("the first round kept its road");

  // A situation note goes on the session as it stands, carrying the block as it now is, and judges nothing.
  remember(
    db,
    AGENT,
    &Leaving {
      chat_id: OWNER,
      route: route.clone(),
      moved: None,
      process: process_mark(db, AGENT).await.unwrap(),
      judged: false,
      resumed: route.session.clone(),
      cache_reads: None,
      input: None,
      text: String::new(),
    },
  )
  .await
  .unwrap();

  let state = unified.standing.receiver.units().agency.state(SESSION).await.unwrap_or_default();

  assert_eq!(
    reset_reason(db, AGENT, &route, &state).await.unwrap(),
    Some(SessionReason::Process),
    "the next round still finds the instructions the session read are not the ones that stand"
  );
}

#[tokio::test]
async fn a_change_that_leaves_the_process_instructions_as_they_were_replaces_nothing() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  process_row(&unified, &["information", "final"], &[]).await;
  unified.round(1, "the first words").await;

  // The same row set again is no change of the block.
  process_row(&unified, &["information", "final"], &[]).await;
  unified.round(2, "the second words").await;
  assert!(unified.standing.agency.ended().is_empty(), "the same row set again replaces nothing");

  // A question switched off and on again before the session's next turn leaves the block as it was.
  process_row(&unified, &["information", "final"], &["information"]).await;
  process_row(&unified, &["information", "final"], &[]).await;
  unified.round(3, "the third words").await;
  assert!(unified.standing.agency.ended().is_empty(), "a change set back replaces nothing");

  // Switched off, it leaves the block, and the session that read its line is replaced.
  process_row(&unified, &["information", "final"], &["information"]).await;
  unified.round(4, "the fourth words").await;
  assert_eq!(unified.standing.agency.ended(), [SESSION]);
}

// A state of the session that names no provider or model — the agency restarting, or a session opened afresh and
// not yet through its first turn — says nothing of the road: it replaces nothing, while another model named does.
#[test]
fn a_state_naming_no_road_moves_no_road() {
  use bmo_receiver::chat::cache::{Route, route_reason};
  use receiver_codes::SessionReason;

  let held = Route {
    provider: "provider-a".to_owned(),
    model: "model-a".to_owned(),
    ..Route::default()
  };

  assert_eq!(route_reason(&held, "", ""), None, "an unread state");
  assert_eq!(route_reason(&held, "provider-a", ""), None, "a state naming no model");
  assert_eq!(route_reason(&held, "provider-a", "model-a"), None);
  assert_eq!(route_reason(&held, "provider-a", "model-b"), Some(SessionReason::Route));
}

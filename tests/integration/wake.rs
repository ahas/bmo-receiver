//! The wake (docs/wake.md, 「Who reaches the session」): whom a message wakes, what stops it before anybody is
//! woken, and the turn each woken agent's conversations are handed. The tests stand on the receiver's shared
//! schema in rooms of their own, reaching the agency's registrations, bmo-job's questions and the web unit's
//! account model through their stand-ins.

mod question;
mod rest;
mod sent_lines;

use std::sync::Arc;

use bmo_chat_client::MessageFile;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::rows;
use bmo_receiver::receiver::wake::agents;
use bmo_receiver::receiver::{Context, Inbound, Room};
use bmo_receiver::roads::added::{self, RoomMode};
use receiver_policy::commands::COMMANDS;
use receiver_ports::agency::{Rest, RestKind};
use receiver_ports::job::{Chosen, Moved, OpenQuestion, Talked};
use receiver_ports::web::UserNode;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::units::{
  ADDRESS, AGENT, Handed, OWNER, StandInChat, Standing, registration, standing, woken,
};

/// The agent's number in the rooms of these tests.
const AGENT_NUMBER: i64 = 7;

fn room() -> Room {
  Room {
    owner_room: false,
    identity: Some(AGENT_NUMBER),
    private_to: None,
    sealed: false,
  }
}

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(AGENT_NUMBER),
    private_to: None,
    sealed: false,
  }
}

fn inbound(chat_id: i64, message_id: i64, sender_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id,
    message_id,
    sender_id: Some(sender_id),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000,
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

// A receiver over a scratch database and data root of its own, reaching one chat service that holds the rooms, each
// on the listen list so the receiver listens to it (docs/wake.md, 「Who reaches the session」).
async fn stood(rooms: &[i64]) -> (Scratch, tempfile::TempDir, Standing) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(rooms));

  let stood = woken(standing(scratch.db(), DataRoot::new(data.path()), vec![chat]));

  for room in rooms {
    rows::allow_chat(scratch.db(), *room).await.unwrap();
  }

  (scratch, data, stood)
}

async fn everyone_heard(scratch: &Scratch) {
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('mode', 'everyone') ON CONFLICT ("key") DO UPDATE SET "value" = excluded."value""#,
      vec![],
    )
    .await;
}

// The window a room holds open for an agent, so a later word of no one's calls it.
async fn opened(scratch: &Scratch, chat_id: i64, agent: &str, until: i64) {
  scratch
    .run(
      r#"INSERT INTO "room_windows" ("chat_id", "agent", "open_until") VALUES ($1, $2, $3)"#,
      vec![chat_id.into(), agent.into(), until.into()],
    )
    .await;
}

// A file handed over with the message it came on, under the caption typed with it where there is one.
async fn sent_file(
  standing: &Standing,
  message: Inbound,
  room: Room,
  caption: Option<&str>,
  file: MessageFile,
) {
  let context = Context {
    spoken_text: caption.map(str::to_owned),
    carries_file: true,
    ..Context::default()
  };

  standing.receiver.attachment(&message, &context, room, &[file]).await.unwrap();
}

fn picture() -> MessageFile {
  MessageFile {
    id: "1".into(),
    name: "screen.jpg".into(),
    mime: "image/jpeg".into(),
    bytes: vec![0xff, 0xd8],
    kind: None,
  }
}

fn document() -> MessageFile {
  MessageFile {
    id: "1".into(),
    name: "a.pdf".into(),
    mime: "application/pdf".into(),
    bytes: b"pdf".to_vec(),
    kind: None,
  }
}

fn asked(id: i64, talks: bool) -> OpenQuestion {
  OpenQuestion {
    id,
    options: ["moved to rust", "left on bun", "both stood"].map(str::to_owned).to_vec(),
    talks,
  }
}

// The owner's words naming the second option, with the suffix the policy allows after a number.
fn choosing() -> String {
  format!("2{}", COMMANDS.option_suffixes[0])
}

#[tokio::test]
async fn the_owner_answering_a_jobs_question_in_his_chat_settles_it_or_talks_over_it_and_wakes_nobody() {
  let (_scratch, _data, standing) = stood(&[OWNER]).await;

  standing.job.asking(OWNER, asked(1, false));
  standing
    .receiver
    .message(&inbound(OWNER, 1, OWNER, &choosing()), &Context::default(), owners_room())
    .await
    .unwrap();

  // Settling is not answering the agent: nothing is woken, and the queue's standing is told as moved.
  assert_eq!(standing.job.settled(), [Chosen { id: 1, chosen: 1 }]);
  assert!(standing.waking.woken().is_empty());
  assert_eq!(standing.job.moved(), [Moved { detail: None }]);

  standing.job.asking(OWNER, asked(2, true));
  standing
    .receiver
    .message(&inbound(OWNER, 2, OWNER, "wait, why that one?"), &Context::default(), owners_room())
    .await
    .unwrap();

  // The talk is written over the question and its clock stopped, and the question stays unsettled.
  assert_eq!(
    standing.job.talked(),
    [Talked {
      id: 2,
      say: "wait, why that one?".into()
    }]
  );
  assert_eq!(standing.job.settled().len(), 1);
  assert!(standing.waking.woken().is_empty());
  assert_eq!(standing.job.moved().len(), 2);
}

#[tokio::test]
async fn talk_the_question_itself_refuses_is_taken_as_an_ordinary_message() {
  for (id, code) in ["question_absent", "question_takes_no_talk", "question_talk_settled"]
    .into_iter()
    .enumerate()
  {
    let (_scratch, _data, standing) = stood(&[OWNER]).await;
    let id = i64::try_from(id).unwrap() + 1;

    standing
      .agency
      .registering([registration(AGENT, "Agent A", vec![ADDRESS], None)].to_vec());
    standing.job.asking(OWNER, asked(id, true));
    standing.job.refusing_talk(code);
    standing
      .receiver
      .message(&inbound(OWNER, id, OWNER, "wait, why that one?"), &Context::default(), owners_room())
      .await
      .unwrap_or_else(|why| panic!("a talk refused {code} failed the message: {why}"));

    // Nothing was written over the question, and the words went on to be answered as any words are.
    assert!(standing.job.talked().is_empty());
    assert!(standing.job.moved().is_empty());
    let woken = standing.taken.handed();

    assert_eq!(woken.len(), 1, "a talk refused {code} was not handed on");
    assert_eq!(woken[0].message_id, id);
  }
}

#[tokio::test]
async fn another_agent_called_is_taken_by_its_own_conversations_and_opens_its_window_and_its_own_words_wake_only_others()
 {
  let (scratch, _data, standing) = stood(&[-121]).await;

  standing.agency.registering(
    [
      registration(AGENT, "Agent A", vec![ADDRESS], None),
      registration("agent-c", "Agent C", vec!["see"], Some(60)),
    ]
    .to_vec(),
  );
  scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES (-121, 'agent-a'), (-121, 'agent-c')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('road-a', 'agent-c', 77) ON CONFLICT DO NOTHING"#,
      vec![],
    )
    .await;

  standing
    .receiver
    .message(&inbound(-121, 1, OWNER, "see that"), &Context::default(), room())
    .await
    .unwrap();

  // Her turn is her own conversations' in the one runner, never the default answerer's, and it opens her window.
  assert_eq!(
    standing.taken.handed(),
    [Handed {
      agent: "agent-c".into(),
      chat_id: -121,
      message_id: 1,
      files: vec![],
      opens_window: true,
      read_through: Some(false),
    }]
  );
  assert!(
    scratch
      .count(
        r#"SELECT "open_until" FROM "room_windows" WHERE "chat_id" = -121 AND "agent" = 'agent-c'"#,
        vec![]
      )
      .await
      > 0
  );

  // Her answer, filed under her own number, calls the default answerer: it wakes him, and her open window does not
  // wake her with her own words.
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES (-121, 2, 77, 'Agent C', 1760000001, 'agent-a, look at this', 'road-a')"#,
      vec![],
    )
    .await;
  standing.receiver.filed(-121, 2, "road-a", room()).await.unwrap();

  let woken = standing.taken.handed();

  assert_eq!(woken.len(), 2);
  assert_eq!(
    woken[1],
    Handed {
      agent: AGENT.into(),
      chat_id: -121,
      message_id: 2,
      files: vec![],
      opens_window: true,
      read_through: Some(false),
    }
  );
}

#[tokio::test]
async fn another_users_private_room_wakes_the_chat_agent_on_every_word_and_a_group_room_still_wants_a_call()
{
  let (scratch, _data, standing) = stood(&[-131, -132, -133]).await;
  let person = 9306;
  let stranger = 9307;
  let one_to_one = |with: i64| Room {
    private_to: Some(with),
    ..room()
  };

  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None)].to_vec());
  everyone_heard(&scratch).await;
  standing.web.linking(person, "user-9".into());
  // The person's own private room stands on the console's room list as any other listened room does.
  rows::allow_chat(scratch.db(), person).await.unwrap();

  standing
    .receiver
    .message(
      &inbound(person, 1, person, "what is for supper"),
      &Context::default(),
      one_to_one(person),
    )
    .await
    .unwrap();
  standing
    .receiver
    .message(&inbound(-132, 1, person, "what is for supper"), &Context::default(), room())
    .await
    .unwrap();
  standing
    .receiver
    .message(
      &inbound(stranger, 1, stranger, "what is for supper"),
      &Context::default(),
      one_to_one(stranger),
    )
    .await
    .unwrap();

  let woken = standing.taken.handed();

  assert_eq!(
    woken,
    [Handed {
      agent: AGENT.into(),
      chat_id: person,
      message_id: 1,
      files: vec![],
      opens_window: false,
      read_through: Some(false),
    }]
  );
}

#[tokio::test]
async fn a_private_room_is_answered_by_the_supervisor_its_person_owns() {
  let (_scratch, _data, standing) = stood(&[-141]).await;
  let person = 9406;

  standing.web.linking(person, "77".to_owned());

  let mut owner = registration("agent-b", "Agent B", vec!["agent-b"], None);
  owner.row.insert("ownerAccount".to_owned(), json!(77));
  standing
    .agency
    .registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None), owner]);

  let room = Room {
    private_to: Some(person),
    ..room()
  };
  let (answerers, alone) = agents::room_designated(&standing.receiver, -141, room).await.unwrap();

  assert_eq!(answerers.len(), 1);
  assert_eq!(answerers[0].name, "agent-b");
  assert!(!answerers[0].default_answerer);
  assert!(alone);
}

#[tokio::test]
async fn a_private_room_whose_person_owns_no_supervisor_falls_back_to_the_default_answerer() {
  let (_scratch, _data, standing) = stood(&[-142]).await;
  let person = 9407;

  standing.web.linking(person, "78".to_owned());

  let mut another = registration("agent-b", "Agent B", vec![], None);
  another.row.insert("ownerAccount".to_owned(), json!(79));
  standing
    .agency
    .registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None), another]);

  let room = Room {
    private_to: Some(person),
    ..room()
  };
  let (answerers, alone) = agents::room_designated(&standing.receiver, -142, room).await.unwrap();

  assert_eq!(answerers.len(), 1);
  assert_eq!(answerers[0].name, AGENT);
  assert!(answerers[0].default_answerer);
  assert!(alone);
}

#[tokio::test]
async fn the_owners_one_to_one_on_a_road_wakes_on_his_words_and_a_room_of_his_account_alone_wants_a_call() {
  let (scratch, _data, standing) = stood(&[-161, -162]).await;
  let his_one_to_one = Room {
    private_to: Some(OWNER),
    ..room()
  };

  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None)].to_vec());
  everyone_heard(&scratch).await;
  standing.web.linking(OWNER, "user-1".into());

  standing
    .receiver
    .message(&inbound(-161, 1, OWNER, "what is for supper"), &Context::default(), his_one_to_one)
    .await
    .unwrap();
  standing
    .receiver
    .message(&inbound(-162, 1, OWNER, "what is for supper"), &Context::default(), room())
    .await
    .unwrap();

  assert_eq!(
    standing.taken.handed(),
    [Handed {
      agent: AGENT.into(),
      chat_id: -161,
      message_id: 1,
      files: vec![],
      opens_window: false,
      read_through: Some(false),
    }]
  );
}

#[tokio::test]
async fn a_group_room_is_answered_by_the_supervisor_its_leader_names() {
  let (scratch, _data, standing) = stood(&[-151]).await;

  let mut leader = registration("agent-b", "Agent B", vec!["agent-b"], None);
  leader.row.insert("ownerAccount".to_owned(), json!(77));
  standing
    .agency
    .registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None), leader]);
  scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES (-151, 'agent-b')"#,
      vec![],
    )
    .await;

  let (answerers, alone) = agents::room_designated(&standing.receiver, -151, room()).await.unwrap();

  assert_eq!(answerers.iter().map(|one| one.name.as_str()).collect::<Vec<_>>(), ["agent-b"]);
  assert!(alone);
}

#[test]
fn a_send_goes_out_in_the_account_of_the_leader_it_speaks_as_or_of_the_one_leader() {
  let named = |name: &str, account: Option<i64>| agents::Designated {
    name: name.to_owned(),
    call_names: vec![],
    window_seconds: None,
    owner_account: account,
    default_answerer: false,
  };
  let two = [named("agent-a", Some(77)), named("agent-b", Some(79)), named("agent-c", None)];
  let one = [named("agent-a", Some(77)), named("agent-d", Some(77)), named("agent-c", None)];

  assert_eq!(agents::sending_account(&two, Some("agent-b")), Some(79));
  assert_eq!(agents::sending_account(&two, Some("agent-c")), None);
  assert_eq!(agents::sending_account(&two, None), None);
  assert_eq!(agents::sending_account(&one, Some("agent-c")), Some(77));
  assert_eq!(agents::sending_account(&one, None), Some(77));
  assert_eq!(agents::sending_account(&[], Some("agent-a")), None);
  assert_eq!(agents::leaders_of(&two), [77, 79]);
}

#[tokio::test]
async fn a_group_room_naming_no_supervisor_stays_with_the_default_answerer() {
  let (_scratch, _data, standing) = stood(&[-152]).await;

  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);

  let (answerers, alone) = agents::room_designated(&standing.receiver, -152, room()).await.unwrap();

  assert_eq!(answerers.len(), 1);
  assert_eq!(answerers[0].name, AGENT);
  assert!(answerers[0].default_answerer);
  assert!(!alone, "a group room naming nobody is not answered alone");
}

#[tokio::test]
async fn the_chat_agents_own_words_filed_in_a_room_wake_nobody_and_a_later_word_in_its_open_window_does() {
  let (scratch, _data, standing) = stood(&[-134]).await;

  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None)].to_vec());
  everyone_heard(&scratch).await;
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES (-134, 1, 7, 'Agent A', 1760000001, 'an answer of my own', 'road-a')"#,
      vec![],
    )
    .await;

  standing.receiver.filed(-134, 1, "road-a", room()).await.unwrap();
  assert!(standing.taken.handed().is_empty());

  // A person's word later, with the window still open, reaches him without a call.
  opened(&scratch, -134, AGENT, 4_102_444_800_000).await;
  standing
    .receiver
    .message(&inbound(-134, 2, 55, "still worth a word"), &Context::default(), room())
    .await
    .unwrap();

  let woken = standing.taken.handed();

  assert_eq!(woken.len(), 1);
  assert_eq!(woken[0].message_id, 2);
}

#[tokio::test]
async fn a_persons_file_rides_the_request_that_follows_it_and_a_muted_person_wakes_nobody() {
  let (scratch, _data, standing) = stood(&[-122]).await;

  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None)].to_vec());
  everyone_heard(&scratch).await;
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES (-122, 1, 55, 'tester', 1760000000, '[file a.pdf]', 'road-a'), (-122, 2, 56, 'other', 1760000010, '[file b.pdf]', 'road-a')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "collected_items" ("chat_id", "message_id", "sender_name", "kind", "source", "stored_path", "collected_at")
         VALUES (-122, 1, 'tester', 'document', 'a.pdf', '/kept/a.pdf', 1), (-122, 2, 'other', 'document', 'b.pdf', '/kept/b.pdf', 1)"#,
      vec![],
    )
    .await;
  scratch
    .run(r#"INSERT INTO "muted_members" ("chat_id", "user_id") VALUES (-122, 56)"#, vec![])
    .await;

  let request = Inbound {
    sent_at: 1_760_000_060,
    ..inbound(-122, 3, 55, "agent-a look at this")
  };

  standing.receiver.message(&request, &Context::default(), room()).await.unwrap();
  standing
    .receiver
    .message(&inbound(-122, 4, 56, "agent-a me too"), &Context::default(), room())
    .await
    .unwrap();

  // The file goes in ahead of the request it rides; the muted person's words wake nobody.
  let woken = standing.taken.handed();

  assert_eq!(woken.len(), 1);
  assert_eq!(woken[0].message_id, 3);
  assert_eq!(woken[0].files, [1]);
}

#[tokio::test]
async fn a_rule_subjects_file_wakes_only_on_the_grounds_words_do_and_rides_the_call_after_it() {
  let (_scratch, _data, standing) = stood(&[-140]).await;

  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None)].to_vec());
  standing.web.granting(-140, 55, "rule".into());

  // The rule names the sender, which is heard in owner-only mode but is no ground: with no caption and the
  // window closed, neither the picture nor the document wakes anybody.
  sent_file(&standing, inbound(-140, 1, 55, "[picture]"), room(), None, picture()).await;
  sent_file(&standing, inbound(-140, 2, 55, "[file a.pdf]"), room(), None, document()).await;
  assert!(standing.taken.handed().is_empty());

  // The same picture under a caption that calls the agent wakes it, the document riding ahead of it.
  sent_file(
    &standing,
    inbound(-140, 3, 55, "[picture] agent-a what is this"),
    room(),
    Some("agent-a what is this"),
    picture(),
  )
  .await;

  let woken = standing.taken.handed();

  assert_eq!(woken.len(), 1);
  assert_eq!(woken[0].message_id, 3);
  assert_eq!(woken[0].files, [2]);
  assert!(woken[0].opens_window);
}

#[tokio::test]
async fn a_picture_wakes_the_agent_inside_the_rooms_open_window_and_in_the_owners_own_chat() {
  let (scratch, _data, standing) = stood(&[-141, OWNER]).await;

  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None)].to_vec());
  everyone_heard(&scratch).await;

  // A member's picture with no caption, the window closed, wakes nobody; inside the open window it does.
  sent_file(&standing, inbound(-141, 1, 56, "[picture]"), room(), None, picture()).await;
  assert!(standing.taken.handed().is_empty());
  opened(&scratch, -141, AGENT, 4_102_444_800_000).await;
  sent_file(&standing, inbound(-141, 2, 56, "[picture]"), room(), None, picture()).await;
  // The owner's picture in the owner's own chat needs no caption.
  sent_file(&standing, inbound(OWNER, 1, OWNER, "[picture]"), owners_room(), None, picture()).await;

  let woken = standing
    .taken
    .handed()
    .into_iter()
    .map(|one| (one.chat_id, one.message_id, one.opens_window))
    .collect::<Vec<(i64, i64, bool)>>();

  assert_eq!(woken, [(-141, 2, false), (OWNER, 1, false)]);
}

#[tokio::test]
async fn words_that_are_nothing_but_the_secret_marker_wake_nobody_and_no_name_is_found_in_it() {
  let (scratch, _data, standing) = stood(&[-142, OWNER]).await;
  let marker = bmo_receiver::secrets::marker();

  // An agent whose wake word stands inside the marker is not called by it.
  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec!["secret"], None)].to_vec());
  everyone_heard(&scratch).await;

  standing
    .receiver
    .message(
      &inbound(OWNER, 1, OWNER, &format!(" {marker}\n{marker} ")),
      &Context::default(),
      owners_room(),
    )
    .await
    .unwrap();
  standing
    .receiver
    .message(&inbound(-142, 1, 56, &format!("look {marker}")), &Context::default(), room())
    .await
    .unwrap();
  assert!(standing.taken.handed().is_empty(), "{:?}", standing.taken.handed());

  // The owner's words beside the marker are words, and his own chat needs no name.
  standing
    .receiver
    .message(
      &inbound(OWNER, 2, OWNER, &format!("use this: {marker}")),
      &Context::default(),
      owners_room(),
    )
    .await
    .unwrap();

  let woken = standing
    .taken
    .handed()
    .into_iter()
    .map(|one| (one.chat_id, one.message_id))
    .collect::<Vec<_>>();

  assert_eq!(woken, [(OWNER, 2)]);
}

#[tokio::test]
async fn a_room_the_rest_closes_hands_its_turn_over_marked_so_and_the_owners_chat_is_never_closed() {
  let (_scratch, _data, standing) = stood(&[-125, OWNER]).await;

  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None)].to_vec());
  standing.agency.at_rest(Rest {
    resting: true,
    until: Some(4_102_444_800),
    kind: Some(RestKind::Outage),
  });

  standing
    .receiver
    .message(&inbound(-125, 1, OWNER, "are you there"), &Context::default(), room())
    .await
    .unwrap();
  standing
    .receiver
    .message(&inbound(OWNER, 1, OWNER, "are you there"), &Context::default(), owners_room())
    .await
    .unwrap();

  // With the agent's road closed by the rest, the closed room's turn is not taken; the owner's chat's is.
  assert_eq!(
    standing.taken.handed(),
    [Handed {
      agent: AGENT.into(),
      chat_id: OWNER,
      message_id: 1,
      files: vec![],
      opens_window: false,
      read_through: Some(false),
    }]
  );

  // The agent's road answering through the rest takes the closed room's turn.
  standing.agency.answering_on_their_roads(vec![AGENT.to_owned()]);
  standing
    .receiver
    .message(&inbound(-125, 2, OWNER, "agent-a, still there"), &Context::default(), room())
    .await
    .unwrap();

  assert_eq!(standing.taken.handed().len(), 2);
}

#[tokio::test]
async fn a_room_added_for_the_user_alone_wakes_no_agent_even_where_one_is_called() {
  let (scratch, _data, standing) = stood(&[-131, -132]).await;

  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None)].to_vec());

  for (chat_id, mode) in [(-131, RoomMode::RecordOnly), (-132, RoomMode::Avatar)] {
    assert!(added::add(scratch.db(), chat_id, "road-a", mode, None).await.unwrap());
  }

  // The owner calls the agent by name in both rooms: the room for the user alone keeps the words and wakes
  // nobody, and the room answered as the user wakes the agent by the one rule.
  for chat_id in [-131, -132] {
    standing
      .receiver
      .message(
        &inbound(chat_id, 1, OWNER, &format!("{ADDRESS}, are you there?")),
        &Context::default(),
        room(),
      )
      .await
      .unwrap();
  }

  let woken: Vec<i64> = standing.taken.handed().iter().map(|handed| handed.chat_id).collect();

  assert_eq!(woken, [-132]);
  assert_eq!(
    scratch.count(r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = -131"#, vec![]).await,
    1
  );
  assert!(
    !bmo_receiver::receiver::wake::wakes(
      &standing.receiver,
      AGENT,
      &inbound(-131, 2, OWNER, ADDRESS),
      room()
    )
    .await
    .unwrap()
  );
}

#[tokio::test]
async fn a_member_granted_ask_calls_their_own_supervisor_into_a_group_room_by_name() {
  let (scratch, _data, standing) = stood(&[-161]).await;
  let member = 9411;

  standing.web.linking(member, "81".to_owned());

  let mut own = registration("agent-c", "Agent C", vec!["agent-c"], None);
  own.row.insert("ownerAccount".to_owned(), json!(81));
  standing
    .agency
    .registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None), own]);
  everyone_heard(&scratch).await;

  // Without the grant the call wakes nobody, since the room names the default answerer alone.
  standing
    .receiver
    .message(&inbound(-161, 1, member, "agent-c, sum this up"), &Context::default(), room())
    .await
    .unwrap();
  assert!(standing.taken.handed().is_empty(), "{:?}", standing.taken.handed());

  // Granted ask in the room, the same call reaches the member's own supervisor.
  standing.web.granting(-161, member, "ask".to_owned());
  standing
    .receiver
    .message(&inbound(-161, 2, member, "agent-c, sum this up"), &Context::default(), room())
    .await
    .unwrap();

  let woken = standing.taken.handed();

  assert_eq!(woken.len(), 1, "{woken:?}");
  assert_eq!(woken[0].message_id, 2);

  // A word that does not name it wakes it not.
  standing
    .receiver
    .message(&inbound(-161, 3, member, "thanks all"), &Context::default(), room())
    .await
    .unwrap();
  assert_eq!(standing.taken.handed().len(), 1);
}

#[tokio::test]
async fn a_supervisor_of_an_account_whose_machine_is_gone_is_woken_nowhere() {
  let (scratch, _data, standing) = stood(&[-162]).await;
  let member = 9412;

  standing.web.linking(member, "82".to_owned());
  standing.web.granting(-162, member, "ask".to_owned());

  let mut own = registration("agent-c", "Agent C", vec!["agent-c"], None);
  own.row.insert("ownerAccount".to_owned(), json!(82));
  standing
    .agency
    .registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None), own]);
  everyone_heard(&scratch).await;

  // Bound to a machine the hub does not have attached, the account's supervisor is woken nowhere.
  standing.web.binding("82", false);
  standing
    .receiver
    .message(&inbound(-162, 1, member, "agent-c, sum this up"), &Context::default(), room())
    .await
    .unwrap();
  assert!(standing.taken.handed().is_empty(), "{:?}", standing.taken.handed());

  // Attached again, the same call reaches it.
  standing.web.binding("82", true);
  standing
    .receiver
    .message(&inbound(-162, 2, member, "agent-c, sum this up"), &Context::default(), room())
    .await
    .unwrap();

  let woken = standing.taken.handed();

  assert_eq!(woken.len(), 1, "{woken:?}");
  assert_eq!(woken[0].message_id, 2);
  // Left, the account's supervisor is woken nowhere again, attached or not.
  standing.web.nodes.lock().unwrap().insert(
    "82".to_owned(),
    UserNode {
      agent: Some("u82".to_owned()),
      attached: true,
      left: true,
    },
  );
  standing
    .receiver
    .message(&inbound(-162, 3, member, "agent-c, sum this up"), &Context::default(), room())
    .await
    .unwrap();
  assert_eq!(standing.taken.handed().len(), 1);
}

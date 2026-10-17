//! The wake (docs/wake.md, 「Who reaches the session」): whom a message wakes, what stops it before anybody is
//! woken, and the turn each woken agent's conversations are handed. The tests stand on the receiver's shared
//! schema in rooms of their own, reaching the agency's registrations, bmo-job's questions and the web unit's
//! account model through their stand-ins.

mod question;
mod rest;

use std::sync::Arc;

use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_policy::commands::COMMANDS;
use receiver_ports::agency::{Rest, RestKind};
use receiver_ports::job::{Chosen, Moved, OpenQuestion, Talked};

use crate::common::Scratch;
use crate::common::units::{
  ADDRESS, AGENT, Handed, OWNER, StandInChat, Standing, registration, standing, woken,
};

/// The agent's number in the rooms of these tests.
const AGENT_NUMBER: i64 = 7;

fn room() -> Room {
  Room {
    owner_room: false,
    identity: Some(AGENT_NUMBER),
    private_to: None,
  }
}

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(AGENT_NUMBER),
    private_to: None,
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
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: Some("bot-account".into()),
    via_reader: None,
    album: None,
  }
}

// A receiver over a scratch database and data root of its own, reaching one chat service that holds the rooms.
async fn stood(rooms: &[i64]) -> (Scratch, tempfile::TempDir, Standing) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(rooms));

  let stood = woken(standing(scratch.db(), DataRoot::new(data.path()), vec![chat]));

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
async fn a_companion_called_is_taken_by_its_own_conversations_and_opens_its_window_and_its_own_words_wake_only_others()
 {
  let (scratch, _data, standing) = stood(&[-121]).await;

  standing.agency.registering(
    [
      registration(AGENT, "Agent A", vec![ADDRESS], None, true),
      registration("agent-c", "Agent C", vec!["see"], Some(60), false),
    ]
    .to_vec(),
  );
  scratch
    .run(
      r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES (-121, 'agent-a'), (-121, 'agent-c')"#,
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

  // Her turn is her own conversations' in the one runner, never the chat agent's, and it opens her window.
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

  // Her answer, filed under her own number, calls the chat agent: it wakes him, and her open window does not
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
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None, true)].to_vec());
  everyone_heard(&scratch).await;
  standing.web.linking(person, "user-9".into());

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
async fn the_chat_agents_own_words_filed_in_a_room_wake_nobody_and_a_later_word_in_its_open_window_does() {
  let (scratch, _data, standing) = stood(&[-134]).await;

  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None, true)].to_vec());
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
  opened(&scratch, -134, AGENT, 4_102_444_800).await;
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
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None, true)].to_vec());
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
async fn a_room_the_rest_closes_hands_its_turn_over_marked_so_and_the_owners_chat_is_never_closed() {
  let (_scratch, _data, standing) = stood(&[-125, OWNER]).await;

  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None, true)].to_vec());
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

  // With no stand-in answering the room, the closed room's turn is not taken; the owner's chat's is.
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

  // A stand-in answering on the agent's road takes the closed room's turn.
  standing.agency.standing_in(vec![AGENT.to_owned()]);
  standing
    .receiver
    .message(&inbound(-125, 2, OWNER, "agent-a, still there"), &Context::default(), room())
    .await
    .unwrap();

  assert_eq!(standing.taken.handed().len(), 2);
}

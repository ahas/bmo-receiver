//! The conversations' own readings: whose speaker an event a round publishes goes out under.

use bmo_receiver::chat::conversation::told_as;

#[test]
fn an_event_of_a_room_goes_out_under_its_speaker_and_one_naming_its_own_keeps_it() {
  let speaker = serde_json::json!({ "id": 11, "name": "agent-b" });
  let progress =
    serde_json::json!({ "type": "run-progress", "chatId": -1003, "progress": { "phase": "tool" } });
  let sent = serde_json::json!({ "type": "sent", "chatId": -1003, "messageId": 5 });
  let draft =
    serde_json::json!({ "type": "draft", "chatId": -1003, "kind": "start", "agentName": "agent-b" });
  let named =
    serde_json::json!({ "type": "sent", "chatId": -1003, "speaker": { "id": 12, "name": "agent-c" } });

  assert_eq!(told_as(progress.clone(), Some(speaker.clone()))["speaker"], speaker);
  assert_eq!(
    told_as(sent, Some(speaker.clone()))["speaker"],
    speaker,
    "a sent line names its speaker"
  );
  assert_eq!(
    told_as(draft, Some(speaker.clone()))["speaker"],
    speaker,
    "a draft row names its speaker"
  );
  assert_eq!(told_as(named.clone(), Some(speaker)), named);
  assert_eq!(told_as(progress.clone(), None), progress);
}

fn said(message_id: i64, sender_id: i64, text: &str) -> bmo_receiver::receiver::Inbound {
  bmo_receiver::receiver::Inbound {
    chat_id: -1003,
    message_id,
    sender_id: Some(sender_id),
    sender_name: format!("person-{sender_id}"),
    sent_at: 1_760_000_000 + message_id,
    text: text.to_owned(),
    image_path: None,
    reply_to_message_id: None,
    via: "road-a".to_owned(),
    mode: None,
    via_reader: None,
    album: None,
  }
}

#[test]
fn the_answer_is_pinned_to_the_message_it_answers_not_to_the_owners_aside_after_it() {
  use bmo_receiver::chat::conversation::{asks_of, pin_target};
  use bmo_receiver::receiver::wake::agents::Designated;

  // The live case's shape: another person's game question, then the owner's aside two seconds later.
  let agent = Designated {
    name: "agent-a".to_owned(),
    call_names: vec!["Agent A".to_owned()],
    window_seconds: None,
    default_answerer: true,
    kind: None,
  };
  let round = [said(20201, 31, "how many tries are left?"), said(20202, 42, "none of them, ha")];
  let asks = |message: &bmo_receiver::receiver::Inbound| asks_of(&agent, message);

  assert_eq!(
    pin_target(&round, None, asks).map(|one| one.message_id),
    Some(20201),
    "the question, not the aside"
  );
  assert_eq!(
    pin_target(&round, Some(20202), asks).map(|one| one.message_id),
    Some(20202),
    "the mark names it"
  );
  assert_eq!(
    pin_target(&round, Some(999), asks).map(|one| one.message_id),
    Some(20201),
    "a mark naming no message of the round names nothing"
  );

  // Calling the agent by name asks something of it; with nothing asked the newest stands.
  let called = [said(1, 31, "fine weather"), said(2, 32, "Agent A, look at this"), said(3, 33, "indeed")];

  assert_eq!(pin_target(&called, None, asks).map(|one| one.message_id), Some(2));
  assert_eq!(pin_target(&called[..1], None, asks).map(|one| one.message_id), Some(1));
  assert_eq!(pin_target(&[], None, asks), None);
}

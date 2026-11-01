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

#[test]
fn waiting_words_join_a_working_round_only_where_they_carry_its_privilege() {
  use bmo_receiver::chat::conversation::joins_round;

  const OWNER: i64 = 1;
  const PERSON: i64 = 2;

  // The owner's round: his words join, an agent's line alone keeps nothing out, another person's words wait.
  assert!(joins_round(true, &[OWNER], OWNER));
  assert!(joins_round(true, &[], OWNER), "an agent's line alone joins and holds nothing back");
  assert!(!joins_round(true, &[OWNER, PERSON], OWNER));

  // A round that is not his, opened by an agent's line alone: his own words wait and open a round of his own.
  assert!(!joins_round(false, &[OWNER], OWNER));
  assert!(joins_round(false, &[PERSON], OWNER));
  assert!(joins_round(false, &[OWNER, PERSON], OWNER));
  assert!(joins_round(false, &[], OWNER));
}

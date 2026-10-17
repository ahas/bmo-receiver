//! The conversations' own readings: whose speaker an event a round publishes goes out under.

use bmo_receiver::chat::conversation::told_as;

#[test]
fn an_event_of_a_companions_room_goes_out_under_its_speaker_and_one_naming_its_own_keeps_it() {
  let speaker = serde_json::json!({ "id": 11, "name": "Companion" });
  let progress =
    serde_json::json!({ "type": "run-progress", "chatId": -1003, "progress": { "phase": "tool" } });
  let named =
    serde_json::json!({ "type": "sent", "chatId": -1003, "speaker": { "id": 12, "name": "Other" } });

  assert_eq!(told_as(progress.clone(), Some(speaker.clone()))["speaker"], speaker);
  assert_eq!(told_as(named.clone(), Some(speaker)), named);
  assert_eq!(told_as(progress.clone(), None), progress);
}

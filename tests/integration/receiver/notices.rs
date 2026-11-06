//! The frame an acknowledgement is handed to the room's session in (docs/voiced-notices.md, 「The rule」).

use bmo_receiver::receiver::notices::{instruction, kind};
use receiver_codes::SettingFact;
use receiver_policy::chat_round::CHAT_ROUND;

#[test]
fn the_frame_carries_the_fact_and_what_the_line_is_for_under_its_marker() {
  let framed = instruction(&SettingFact::PhotosSwitched { on: false }.to_string());

  assert!(framed.starts_with(&CHAT_ROUND.notice.marker), "{framed}");
  assert!(framed.contains(
    "\nThe owner switched photo viewing off for this chat alone: you know when a picture arrives here but do \
     not open it. Other chats are unchanged.\nEND OF FACTS\n"
  ));
  assert!(framed.contains("acknowledge the owner's command in this chat."), "{framed}");
  assert!(
    !framed.contains("in this chat's own language") && !framed.contains("Do not answer <<SKIP>>"),
    "the rule stands in the room's block, not the turn: {framed}"
  );
}

#[test]
fn every_acknowledgement_of_one_setting_stands_under_one_kind() {
  assert_eq!(
    kind(&SettingFact::RestartNoticeSwitched { on: false }),
    kind(&SettingFact::RestartNoticeSwitched { on: true })
  );
  assert_eq!(kind(&SettingFact::MuteTargetMissing), "setting:mute");
  assert_ne!(kind(&SettingFact::PhotosSwitched { on: true }), kind(&SettingFact::ChatAllowed));
}

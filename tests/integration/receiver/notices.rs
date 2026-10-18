//! The frame an acknowledgement is handed to the room's session in (docs/voiced-notices.md, 「The rule」).

use bmo_receiver::receiver::notices::{instruction, kind};
use receiver_codes::SettingFact;

#[test]
fn the_frame_carries_the_fact_and_says_the_line_is_to_be_said_in_the_chats_own_language() {
  let framed = instruction(&SettingFact::ToolsSwitched { on: false }.to_string());

  assert!(framed.starts_with("The service has something to tell this chat"));
  assert!(framed.contains(
    "\nThe owner switched tools off in this chat: here you now answer in words only.\nEND OF FACTS\n"
  ));
  assert!(framed.contains("What the line is for: acknowledge the owner's command in this chat."));
  assert!(framed.contains("in this chat's own language"));
  assert!(framed.ends_with("Do not answer <<SKIP>>: this line is to be said."));
}

#[test]
fn every_acknowledgement_of_one_setting_stands_under_one_kind() {
  assert_eq!(kind(&SettingFact::RestNoticeKept), kind(&SettingFact::RestNoticeSwitched { on: true }));
  assert_eq!(kind(&SettingFact::MuteTargetMissing), "setting:mute");
  assert_ne!(
    kind(&SettingFact::ToolsSwitched { on: true }),
    kind(&SettingFact::PhotosSwitched { on: true })
  );
}

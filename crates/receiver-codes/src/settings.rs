//! The owner's in-room setting commands (docs/rooms.md): the fact each leaves the room's session to acknowledge
//! in its own voice, once the setting has changed.

use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

/// One setting's new state, as the room's session is told it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SettingFact {
  /// The owner allowed the room: it is listened to and its record kept from now on.
  ChatAllowed,
  /// Opening the room's pictures, switched for this room alone.
  PhotosSwitched {
    on: bool,
  },
  RestartNoticeSwitched {
    on: bool,
  },
  /// A member muted, or unmuted.
  MemberMuted {
    muted: bool,
  },
  /// A mute or unmute that named nobody who can be.
  MuteTargetMissing,
}

coded!(SettingFact);

impl SettingFact {
  /// The setting the fact is about: one acknowledgement waits per setting, so a setting switched twice before the
  /// session speaks leaves one line.
  pub fn setting(&self) -> &'static str {
    match self {
      Self::ChatAllowed => "chat-allowed",
      Self::PhotosSwitched { .. } => "photos",
      Self::RestartNoticeSwitched { .. } => "notice-restart",
      Self::MemberMuted { .. } | Self::MuteTargetMissing => "mute",
    }
  }
}

fn switched(on: bool) -> &'static str {
  if on { "on" } else { "off" }
}

impl std::fmt::Display for SettingFact {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::ChatAllowed => {
        f.write_str("The owner has allowed this chat: from now on you listen to it and keep its record.")
      }
      Self::PhotosSwitched { on: true } => f.write_str(
        "The owner switched photo viewing on for this chat alone: you open the pictures posted here and answer \
         them. Other chats are unchanged.",
      ),
      Self::PhotosSwitched { on: false } => f.write_str(
        "The owner switched photo viewing off for this chat alone: you know when a picture arrives here but do \
         not open it. Other chats are unchanged.",
      ),
      Self::RestartNoticeSwitched { on } => {
        write!(f, "The owner switched restart notices {} in this chat.", switched(*on))
      }
      Self::MemberMuted { muted: false } => {
        f.write_str("The owner unmuted a member of this chat: you answer that person here again.")
      }
      Self::MemberMuted { muted: true } => {
        f.write_str("The owner muted a member of this chat: you no longer answer that person here.")
      }
      Self::MuteTargetMissing => f.write_str(
        "The owner asked to mute or unmute someone, but the command named nobody that can be: it has to reply to \
         that person's message or give their numeric id, and neither the owner nor you can be muted. Nothing \
         changed.",
      ),
    }
  }
}

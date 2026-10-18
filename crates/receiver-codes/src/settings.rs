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
  ToolsSwitched {
    on: bool,
  },
  /// Opening the room's pictures, switched for this room alone.
  PhotosSwitched {
    on: bool,
  },
  /// Delegation switched, naming the delegate by its registered label where the deployment names one.
  DelegationSwitched {
    on: bool,
    delegate: Option<String>,
  },
  RestNoticeSwitched {
    on: bool,
  },
  /// The owner asked to switch the rest notice off in the owner's own private chat, where it stays on.
  RestNoticeKept,
  RestartNoticeSwitched {
    on: bool,
  },
  /// A member muted, or unmuted.
  MemberMuted {
    muted: bool,
  },
  /// A mute or unmute that named nobody who can be.
  MuteTargetMissing,
  /// The mode the whole deployment listens in: everyone's words, or the owner's alone.
  ModeSet {
    everyone: bool,
  },
}

coded!(SettingFact);

impl SettingFact {
  /// The setting the fact is about: one acknowledgement waits per setting, so a setting switched twice before the
  /// session speaks leaves one line.
  pub fn setting(&self) -> &'static str {
    match self {
      Self::ChatAllowed => "chat-allowed",
      Self::ToolsSwitched { .. } => "tools",
      Self::PhotosSwitched { .. } => "photos",
      Self::DelegationSwitched { .. } => "delegate",
      Self::RestNoticeSwitched { .. } | Self::RestNoticeKept => "notice-rest",
      Self::RestartNoticeSwitched { .. } => "notice-restart",
      Self::MemberMuted { .. } | Self::MuteTargetMissing => "mute",
      Self::ModeSet { .. } => "mode",
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
      Self::ToolsSwitched { on: true } => f.write_str(
        "The owner switched tools on in this chat: you can now run things and look things up here.",
      ),
      Self::ToolsSwitched { on: false } => {
        f.write_str("The owner switched tools off in this chat: here you now answer in words only.")
      }
      Self::PhotosSwitched { on: true } => f.write_str(
        "The owner switched photo viewing on for this chat alone: you open the pictures posted here and answer \
         them. Other chats are unchanged.",
      ),
      Self::PhotosSwitched { on: false } => f.write_str(
        "The owner switched photo viewing off for this chat alone: you know when a picture arrives here but do \
         not open it. Other chats are unchanged.",
      ),
      Self::DelegationSwitched {
        on: true,
        delegate: Some(delegate),
      } => write!(
        f,
        "The owner switched delegation on in this chat: work {delegate} does better is handed to {delegate}."
      ),
      Self::DelegationSwitched {
        on: true,
        delegate: None,
      } => f.write_str(
        "The owner switched delegation on in this chat: work the delegate does better is handed to it.",
      ),
      Self::DelegationSwitched { on: false, .. } => f.write_str(
        "The owner switched delegation off in this chat: you now do all the work here yourself.",
      ),
      Self::RestNoticeKept => f.write_str(
        "The owner asked to switch rest notices off in their own private chat. That cannot be done there, since \
         it is how they learn why you have gone quiet everywhere else; the notices stay on.",
      ),
      Self::RestNoticeSwitched { on: true } => f.write_str(
        "The owner switched rest notices on in this chat: you will say so here when you go to rest and when you \
         come back.",
      ),
      Self::RestNoticeSwitched { on: false } => f.write_str(
        "The owner switched rest notices off in this chat: when you go to rest, you go quietly here.",
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
      Self::ModeSet { everyone: false } => f.write_str(
        "The owner set the room's mode to owner_only: you now follow the owner's commands alone.",
      ),
      Self::ModeSet { everyone: true } => f.write_str(
        "The owner set the room's mode to everyone: you now listen to everyone and answer them.",
      ),
    }
  }
}

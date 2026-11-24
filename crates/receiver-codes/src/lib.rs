//! The receiver's codes (docs/codes.md): every refusal it answers, every fact it hands a room's agent and every
//! fixed line a room hears, as enums of stable kebab-case codes with structured fields, and the kinds those
//! fields take. None carries a sentence in a room's language: a refusal carries the one English sentence its
//! caller reads beside its code, a fact the English its agent reads, and a fixed line only its code and fields,
//! which the web unit's language packs render.

use std::fmt::Display;

use http::StatusCode;
use serde::{Deserialize, Serialize};

/// Implements [`Code`] for each family as the kebab-case name of its variant.
macro_rules! coded {
  ($($family:ty),+ $(,)?) => {
    $(
      impl $crate::Code for $family {
        fn code(&self) -> &'static str {
          self.into()
        }
      }
    )+
  };
}

/// Implements [`Refused`] for each family from the status each of its variants answers with.
macro_rules! refused {
  ($($family:ty),+ $(,)?) => {
    $(
      impl $crate::Refused for $family {
        fn status(&self) -> http::StatusCode {
          self.answers()
        }
      }
    )+
  };
}

pub mod avatars;
pub mod draws;
pub mod file_fills;
pub mod fixed_lines;
pub mod health;
pub mod kinds;
pub mod message_acts;
pub mod owner_sends;
pub mod room_accounts;
pub mod room_addition;
pub mod room_grants;
pub mod room_members;
pub mod room_mode;
pub mod room_reads;
pub mod room_removal;
pub mod sends;
pub mod settings;
pub mod supervisors;
pub mod talk;
pub mod voice;

pub use avatars::AvatarRefusal;
pub use draws::{DrawAsked, DrawLine, DrawResult};
pub use file_fills::FileFillRefusal;
pub use fixed_lines::FixedLine;
pub use health::HealthFact;
pub use kinds::{HandoverCause, RunTitle, SessionReason, SkipReason, TurnOpening};
pub use message_acts::MessageActRefusal;
pub use owner_sends::{FileUnsent, OwnerSendRefusal};
pub use room_accounts::RoomAccountRefusal;
pub use room_addition::RoomAdditionRefusal;
pub use room_grants::RoomGrantRefusal;
pub use room_members::RoomMembersRefusal;
pub use room_mode::RoomModeRefusal;
pub use room_reads::RoomReadRefusal;
pub use room_removal::{RemovalLeft, RoomRemovalRefusal, RoomRemovalWarning};
pub use sends::SendRefusal;
pub use settings::SettingFact;
pub use supervisors::{SupervisorFact, SupervisorRefusal};
pub use talk::TalkRefusal;
pub use voice::{VoiceFact, VoiceRefusal};

/// A refusal, a fact or a fixed line, named by its code.
pub trait Code {
  fn code(&self) -> &'static str;
}

/// A refusal, which answers its own status beside its code and its English sentence.
pub trait Refused: Code + Display {
  fn status(&self) -> StatusCode;
}

/// Every refusal the receiver answers, each area's under its own variant.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReceiverError {
  #[error(transparent)]
  FileFill(#[from] FileFillRefusal),
  #[error(transparent)]
  Supervisor(#[from] SupervisorRefusal),
  #[error(transparent)]
  OwnerSend(#[from] OwnerSendRefusal),
  #[error(transparent)]
  RoomAccount(#[from] RoomAccountRefusal),
  #[error(transparent)]
  RoomAddition(#[from] RoomAdditionRefusal),
  #[error(transparent)]
  RoomGrant(#[from] RoomGrantRefusal),
  #[error(transparent)]
  RoomMembers(#[from] RoomMembersRefusal),
  #[error(transparent)]
  RoomMode(#[from] RoomModeRefusal),
  #[error(transparent)]
  RoomRead(#[from] RoomReadRefusal),
  #[error(transparent)]
  RoomRemoval(#[from] RoomRemovalRefusal),
  #[error(transparent)]
  Send(#[from] SendRefusal),
  #[error(transparent)]
  Talk(#[from] TalkRefusal),
  #[error(transparent)]
  Voice(#[from] VoiceRefusal),
}

impl ReceiverError {
  fn inner(&self) -> &dyn Refused {
    match self {
      Self::FileFill(refusal) => refusal,
      Self::Supervisor(refusal) => refusal,
      Self::OwnerSend(refusal) => refusal,
      Self::RoomAccount(refusal) => refusal,
      Self::RoomAddition(refusal) => refusal,
      Self::RoomGrant(refusal) => refusal,
      Self::RoomMembers(refusal) => refusal,
      Self::RoomMode(refusal) => refusal,
      Self::RoomRead(refusal) => refusal,
      Self::RoomRemoval(refusal) => refusal,
      Self::Send(refusal) => refusal,
      Self::Talk(refusal) => refusal,
      Self::Voice(refusal) => refusal,
    }
  }
}

impl Code for ReceiverError {
  fn code(&self) -> &'static str {
    self.inner().code()
  }
}

impl Refused for ReceiverError {
  fn status(&self) -> StatusCode {
    self.inner().status()
  }
}

/// Every fact the receiver hands a room's agent to say in its own words, read back by its code alone. Its
/// `Display` is the English the agent reads in its quiet instruction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Fact {
  Supervisor(SupervisorFact),
  Health(HealthFact),
  Voice(VoiceFact),
  Setting(SettingFact),
}

impl Fact {
  fn inner(&self) -> &dyn Code {
    match self {
      Self::Supervisor(fact) => fact,
      Self::Health(fact) => fact,
      Self::Voice(fact) => fact,
      Self::Setting(fact) => fact,
    }
  }
}

impl Code for Fact {
  fn code(&self) -> &'static str {
    self.inner().code()
  }
}

impl Display for Fact {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Supervisor(fact) => fact.fmt(f),
      Self::Health(fact) => fact.fmt(f),
      Self::Voice(fact) => fact.fmt(f),
      Self::Setting(fact) => fact.fmt(f),
    }
  }
}

/// A list laid out for an English sentence, its members joined by commas.
pub(crate) fn listed<T: Display>(members: &[T]) -> String {
  members.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
}

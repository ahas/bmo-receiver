//! A companion added on the console with a room of its own, and removed there (docs/boundary-api.md,
//! 「Companions with a room of their own」).

use http::StatusCode;
use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum CompanionRefusal {
  #[error("{agent} is not a companion")]
  NotCompanion { agent: String },
  #[error(
    "a name starts with a lower-case letter and holds lower-case letters, digits and hyphens, at most 40"
  )]
  Name,
  #[error("the name {name} is another agent's already")]
  NameTaken { name: String },
  #[error("a companion needs a label")]
  Label,
  #[error("the provider {provider} cannot carry a companion")]
  Provider { provider: String },
  #[error("the model is chosen among the registered models that have an address")]
  Model,
  #[error("the worker is chosen among the workers: {worker}")]
  Worker { worker: String },
  #[error("the room needs a title")]
  Room,
  #[error("{label} is a seeded companion, so it cannot be removed")]
  SeededCompanion { label: String },
  #[error("the asked action is not one this route takes")]
  Action,
}

impl CompanionRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::NotCompanion { .. } => StatusCode::NOT_FOUND,
      Self::NameTaken { .. } | Self::SeededCompanion { .. } => StatusCode::CONFLICT,
      Self::Name
      | Self::Label
      | Self::Provider { .. }
      | Self::Model
      | Self::Worker { .. }
      | Self::Room
      | Self::Action => StatusCode::BAD_REQUEST,
    }
  }
}

coded!(CompanionRefusal);
refused!(CompanionRefusal);

/// What the owner is told of a companion's room, to hear in the answer to the console's add.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum CompanionFact {
  /// The companion took back a room already standing under its name, retitled; its messages and documents stay.
  RoomTakenBack { label: String, title: String },
}

coded!(CompanionFact);

impl std::fmt::Display for CompanionFact {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::RoomTakenBack { label, title } => write!(
        f,
        "{label} took back the room already standing under its name; the room is now titled {title}, and its \
         messages and documents stay"
      ),
    }
  }
}

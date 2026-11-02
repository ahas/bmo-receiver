//! An agent added on the console with a room of its own, and removed there (docs/api.md, 「Agents added on the
//! console」). Two codes keep the names the console's language pack matches them by.

use http::StatusCode;
use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum AddedAgentRefusal {
  #[error("{agent} is not an agent added on the console")]
  #[strum(serialize = "not-companion")]
  NotAdded { agent: String },
  #[error(
    "a name starts with a lower-case letter and holds lower-case letters, digits and hyphens, at most 40"
  )]
  Name,
  #[error("the name {name} is another agent's already")]
  NameTaken { name: String },
  #[error("an agent needs a label")]
  Label,
  #[error("the provider {provider} cannot carry a chat agent")]
  Provider { provider: String },
  #[error("the model is chosen among the registered models that have an address")]
  Model,
  #[error("the repository is chosen among the agency's repositories: {worker}")]
  Worker { worker: String },
  #[error("the room needs a title")]
  Room,
  #[error("the agency refused the order at {member}")]
  Order { member: String },
  #[error("the quota share is a fraction from 0 to 1")]
  QuotaShare,
  #[error("{label} is a seeded agent, so it cannot be removed")]
  #[strum(serialize = "seeded-companion")]
  Seeded { label: String },
  #[error("the asked action is not one this route takes")]
  Action,
}

impl AddedAgentRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::NotAdded { .. } => StatusCode::NOT_FOUND,
      Self::NameTaken { .. } | Self::Seeded { .. } => StatusCode::CONFLICT,
      Self::Name
      | Self::Label
      | Self::Provider { .. }
      | Self::Model
      | Self::Worker { .. }
      | Self::Room
      | Self::Order { .. }
      | Self::QuotaShare
      | Self::Action => StatusCode::BAD_REQUEST,
    }
  }
}

coded!(AddedAgentRefusal);
refused!(AddedAgentRefusal);

/// What the owner is told of an added agent's room, to hear in the answer to the console's add.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum AddedAgentFact {
  /// The agent took back a room already standing under its name, retitled; its messages and documents stay.
  RoomTakenBack { label: String, title: String },
}

coded!(AddedAgentFact);

impl std::fmt::Display for AddedAgentFact {
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

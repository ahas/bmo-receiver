//! The supervisor routes and a room's list of supervisors (docs/api.md, 「Supervisors」): a supervisor added with a
//! room of its own, removed, started afresh, and the names a room's list may hold.

use http::StatusCode;
use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum SupervisorRefusal {
  #[error("the agency registers no supervisor {supervisor}")]
  UnknownSupervisor { supervisor: String },
  #[error(
    "a name starts with a lower-case letter and holds lower-case letters, digits and hyphens, at most 40"
  )]
  Name,
  #[error("the name {name} is another supervisor's already")]
  NameTaken { name: String },
  #[error("a supervisor needs a label")]
  Label,
  #[error("the provider {provider} cannot carry the chat line")]
  Provider { provider: String },
  #[error("the model is chosen among the registered models that have an address")]
  Model,
  #[error("the repository is chosen among the agency's repositories: {repository}")]
  Repository { repository: String },
  #[error("the room needs a title")]
  Room,
  #[error("the agency refused the order at {member}")]
  Order { member: String },
  #[error("the quota share is a fraction from 0 to 1")]
  QuotaShare,
  #[error("{label} is seeded by the deployment, so it cannot be removed")]
  Seeded { label: String },
  #[error("{label} is the default answerer, so it cannot be removed until another supervisor is")]
  DefaultAnswerer { label: String },
  #[error("{label} is running a turn now; removing it anyway asks force")]
  Running { label: String },
  #[error("the asked action is not one this route takes")]
  Action,
}

impl SupervisorRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::UnknownSupervisor { .. } => StatusCode::NOT_FOUND,
      Self::NameTaken { .. }
      | Self::Seeded { .. }
      | Self::DefaultAnswerer { .. }
      | Self::Running { .. } => StatusCode::CONFLICT,
      Self::Name
      | Self::Label
      | Self::Provider { .. }
      | Self::Model
      | Self::Repository { .. }
      | Self::Room
      | Self::Order { .. }
      | Self::QuotaShare
      | Self::Action => StatusCode::BAD_REQUEST,
    }
  }
}

coded!(SupervisorRefusal);
refused!(SupervisorRefusal);

/// What the owner is told of a new supervisor's room, to hear in the answer to the console's add.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SupervisorFact {
  /// The supervisor took back a room already standing under its name, retitled; its messages and documents stay.
  RoomTakenBack { label: String, title: String },
}

coded!(SupervisorFact);

impl std::fmt::Display for SupervisorFact {
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

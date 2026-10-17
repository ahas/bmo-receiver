//! A console talk that came back with nothing (bmo-job's `docs/api.md`, 「Talks」): the three codes bmo-job names
//! with their statuses, and the receiver's own for every other way a talk's turn ends unanswered.

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum TalkRefusal {
  #[error("the agent is still answering the talk's last words")]
  Busy,
  #[error("the agent is resting, so these words were not answered")]
  Resting,
  #[error("the provider's usage limit is reached, so these words were not answered")]
  Limit,
  #[error("the provider is not answering, so these words were not answered")]
  Outage,
  #[error("the agent said nothing to these words")]
  Silent,
  #[error("no answer came to these words")]
  Unanswered,
}

impl TalkRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::Busy | Self::Resting => StatusCode::CONFLICT,
      Self::Limit => StatusCode::TOO_MANY_REQUESTS,
      Self::Outage => StatusCode::SERVICE_UNAVAILABLE,
      Self::Silent | Self::Unanswered => StatusCode::BAD_GATEWAY,
    }
  }
}

coded!(TalkRefusal);
refused!(TalkRefusal);

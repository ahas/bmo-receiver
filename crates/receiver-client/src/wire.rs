//! The common wire format (`docs/api.md`, 「The wire」): one page, one error shape and the failure a call came to.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A page of a collection: its members, and whether anything stands past the last of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
  pub items: Vec<T>,
  pub more: bool,
}

impl<T> Page<T> {
  pub fn of(items: Vec<T>) -> Self {
    Self { items, more: false }
  }
}

/// The body every non-2xx answer carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WireError {
  pub code: String,
  pub error: String,
}

/// Why a call came to no answer of the route's shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientFailure {
  /// The service answered with a failure: its status, and its error shape where it gave one.
  Answered {
    status: u16,
    answer: Option<WireError>,
  },
  /// Nothing answered, or the connection did not carry the call.
  Unreachable(String),
  /// The service answered in a shape the route does not give.
  Unreadable(String),
}

impl std::fmt::Display for ClientFailure {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Answered {
        status,
        answer: Some(answer),
      } => write!(out, "the receiver answered {status} {}: {}", answer.code, answer.error),
      Self::Answered {
        status,
        answer: None,
      } => write!(out, "the receiver answered {status}"),
      Self::Unreachable(why) => write!(out, "the receiver did not answer: {why}"),
      Self::Unreadable(why) => write!(out, "the receiver's answer did not read: {why}"),
    }
  }
}

impl std::error::Error for ClientFailure {}

/// What a call answers.
pub type Answer<T> = Result<T, ClientFailure>;

//! What a call is answered with: the protocol's tool result, its text pieces, whether it is a refusal, and the
//! object a tool with an output schema answers beside its text.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One piece of an answer: the receiver's tools answer text alone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Content {
  Text { text: String },
}

/// A tool's answer. A refusal is an answer marked as an error, which the model reads as it reads any other.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Answer {
  pub content: Vec<Content>,
  #[serde(rename = "isError", default)]
  pub is_error: bool,
  #[serde(rename = "structuredContent", default, skip_serializing_if = "Option::is_none")]
  pub structured: Option<Value>,
}

impl Answer {
  pub fn text(text: impl Into<String>) -> Self {
    Self {
      content: vec![Content::Text { text: text.into() }],
      is_error: false,
      structured: None,
    }
  }

  /// What the tool would not do, said as it is: marked as an error.
  pub fn refusal(text: impl Into<String>) -> Self {
    Self {
      is_error: true,
      ..Self::text(text)
    }
  }

  /// An object as a tool with an output schema answers it: the object, and the same object as its text.
  pub fn structured(object: Value) -> Self {
    Self {
      content: vec![Content::Text {
        text: object.to_string(),
      }],
      is_error: false,
      structured: Some(object),
    }
  }

  /// A tool that could not reach what it answers out of: the reason, marked as an error.
  pub fn failure(reason: impl std::fmt::Display) -> Self {
    Self::refusal(format!("ERROR: {reason}"))
  }

  /// The text of every piece, joined, as a reader of text alone reads the answer.
  pub fn plain(&self) -> String {
    self
      .content
      .iter()
      .map(|Content::Text { text }| text.as_str())
      .collect::<Vec<_>>()
      .join("\n")
  }
}

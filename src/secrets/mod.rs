//! The secret-shaped runs a message's words are read for before anything keeps or forwards them
//! (docs/messages.md, 「Secrets taken out」): each run of a shape the base library `bmo-secret-shapes` holds is
//! replaced by its marker, the shapes read in the order the library lists them. The marker is no word to the
//! wake. What this hands back carries no value it took out, and nothing here writes a word of what it read.

pub mod kept;

use bmo_secret_shapes::MARKER;
use serde_json::Value;

/// The marker a run is replaced by.
pub fn marker() -> &'static str {
  MARKER
}

/// Words read for secret-shaped runs: the words with each run replaced, and how many runs there were.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redacted {
  pub text: String,
  pub runs: usize,
}

/// The words with each secret-shaped run replaced by the marker.
pub fn redact(text: &str) -> Redacted {
  let read = bmo_secret_shapes::redact(text, marker());

  Redacted {
    runs: read.runs(),
    text: read.text.into_owned(),
  }
}

/// The words with each secret-shaped run replaced, where only the words matter.
pub fn redacted(text: &str) -> String {
  redact(text).text
}

/// Every string a JSON value holds, its keys aside, read as words are; how many runs they lost.
pub fn redact_value(value: &mut Value) -> usize {
  match value {
    Value::String(text) => {
      let read = redact(text);

      if read.runs > 0 {
        *text = read.text;
      }

      read.runs
    }
    Value::Array(items) => items.iter_mut().map(redact_value).sum(),
    Value::Object(fields) => fields.values_mut().map(redact_value).sum(),
    Value::Null | Value::Bool(_) | Value::Number(_) => 0,
  }
}

/// Whether the words hold the marker.
pub fn marked(text: &str) -> bool {
  text.contains(marker())
}

/// The words as the wake reads them: every marker taken out, since the marker is no word.
pub fn unmarked(text: &str) -> String {
  text.replace(marker(), " ")
}

/// Whether the words are nothing but markers: words a secret alone was pasted as, which wake nobody.
pub fn only_marked(text: &str) -> bool {
  marked(text) && unmarked(text).trim().is_empty()
}

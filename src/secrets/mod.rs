//! The secret-shaped runs a message's words are read for before anything keeps or forwards them
//! (docs/messages.md, 「Secrets taken out」): each run of a shape the policy's `secret-shapes.toml` holds is
//! replaced by the policy's marker, the shapes read in the order the file lists them. The marker is no word to
//! the wake. What this hands back carries no value it took out, and nothing here writes a word of what it read.

pub mod kept;

use std::sync::LazyLock;

use receiver_policy::secret_shapes::SECRET_SHAPES;
use regex::Regex;
use serde_json::Value;

/// The part of a shape's run that alone is replaced, where the shape names one.
const SECRET_PART: &str = "secret";

// One shape compiled: its name, its pattern and the runs it spares.
struct Shape {
  name: &'static str,
  pattern: Regex,
  spared: Vec<Regex>,
}

impl Shape {
  // Whether a run of the shape lies wholly inside a run of one it spares, in the words it was found in.
  fn spares(&self, text: &str, run: &regex::Match) -> bool {
    self.spared.iter().any(|spared| {
      spared
        .find_iter(text)
        .any(|kept| kept.start() <= run.start() && run.end() <= kept.end())
    })
  }
}

// The shapes compiled, in the policy's order.
static SHAPES: LazyLock<Vec<Shape>> = LazyLock::new(|| {
  SECRET_SHAPES
    .shapes
    .iter()
    .map(|shape| {
      let compiled = |pattern: &str| {
        Regex::new(pattern)
          .unwrap_or_else(|why| panic!("the secret shape {} does not compile: {why}", shape.name))
      };

      Shape {
        name: shape.name.as_str(),
        pattern: compiled(&shape.pattern),
        spared: shape.spared.iter().map(|pattern| compiled(pattern)).collect(),
      }
    })
    .collect()
});

/// Every shape compiled now, so a shape that does not compile stops the start rather than a message.
pub fn ready() {
  LazyLock::force(&SHAPES);
}

/// The marker a run is replaced by.
pub fn marker() -> &'static str {
  &SECRET_SHAPES.marker
}

/// The shapes' names, in the order they are read.
pub fn names() -> Vec<&'static str> {
  SHAPES.iter().map(|shape| shape.name).collect()
}

/// Words read for secret-shaped runs: the words with each run replaced, and how many runs there were.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Redacted {
  pub text: String,
  pub runs: usize,
}

/// The words with each secret-shaped run replaced by the marker.
pub fn redact(text: &str) -> Redacted {
  let marker = marker();
  let mut text = text.to_owned();
  let mut runs = 0;

  for shape in SHAPES.iter() {
    if !shape.pattern.is_match(&text) {
      continue;
    }

    let mut out = String::with_capacity(text.len());
    let mut last = 0;

    for found in shape.pattern.captures_iter(&text) {
      let Some(run) = found.name(SECRET_PART).or_else(|| found.get(0)) else {
        continue;
      };

      if run.is_empty() || run.as_str() == marker || shape.spares(&text, &run) {
        continue;
      }

      out.push_str(&text[last..run.start()]);
      out.push_str(marker);
      last = run.end();
      runs += 1;
    }

    out.push_str(&text[last..]);
    text = out;
  }

  Redacted { text, runs }
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

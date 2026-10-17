//! The room line a fair draw posts (docs/random-draw.md, 「The room line」), as its fields.

use serde::{Deserialize, Serialize};

/// Everything a draw's room line carries, in the order the line states it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrawLine {
  /// What the draw decides, where one was given.
  pub label: Option<String>,
  /// What was asked.
  pub asked: DrawAsked,
  /// What came of it.
  pub result: DrawResult,
  /// The draw's number.
  pub number: i64,
  /// The seed it consumed, in hexadecimal.
  pub seed: String,
  /// The method's name.
  pub method: String,
  /// The commitment this seed answers, or none on the room's first draw.
  pub answered: Option<String>,
  /// The commitment to the room's next seed.
  pub next: String,
}

/// What a draw asked, by its shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum DrawAsked {
  Dice {
    count: i64,
    sides: i64,
    modifier: i64,
  },
  Range {
    min: i64,
    max: i64,
  },
  Pick {
    items: usize,
    take: i64,
  },
  Shuffle {
    items: usize,
  },
}

/// What a draw came to, by its shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum DrawResult {
  Dice { faces: Vec<i64>, total: i64 },
  Range { value: i64 },
  Pick { picked: Vec<String> },
  Shuffle { order: Vec<String> },
}

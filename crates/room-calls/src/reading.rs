//! What a reading tool of the receiver's answers with (docs/chat-mcp.md, 「The catalogue」): its reading as lines,
//! under the declared output schema and as its text alike, cut to whole lines under the ceiling every answer of
//! ours stands under, with how to ask for the rest said where they were.

use serde_json::{Value, json};

/// The most characters one answer holds before it is cut.
pub const MAX_ANSWER_CHARS: usize = 30_000;

/// How much of a single long value stands before it is cut, where one value is the whole answer.
pub const MAX_VALUE_CHARS: usize = 2_000;

/// How a verb names what it cut, in its own terms: the unit it counts, how to ask for the rest, and the
/// ceiling where it is not the standing one.
#[derive(Clone, Copy, Debug)]
pub struct Said<'a> {
  pub unit: &'a str,
  pub how: &'a str,
  pub limit: usize,
}

impl<'a> Said<'a> {
  pub fn new(unit: &'a str, how: &'a str) -> Self {
    Self {
      unit,
      how,
      limit: MAX_ANSWER_CHARS,
    }
  }
}

/// What fitted under the ceiling, and what had to be left out.
#[derive(Debug, PartialEq)]
pub struct Capped<'a, T> {
  /// The run of items from the front that fits, in the order they were given.
  pub kept: &'a [T],
  /// How many did not; nought where everything fitted.
  pub left: usize,
  /// The one line saying so, or none where nothing was left out.
  pub note: Option<String>,
}

// Characters are code points, the way every size here is counted, so an answer in any script is measured in
// the units its reader counts in.
fn chars(text: &str) -> usize {
  text.chars().count()
}

/// A count with the thousands mark, as every count here prints carries it.
pub fn count_text(value: usize) -> String {
  let digits = value.to_string();
  let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);

  for (at, digit) in digits.chars().enumerate() {
    if at > 0 && (digits.len() - at).is_multiple_of(3) {
      grouped.push(',');
    }

    grouped.push(digit);
  }

  grouped
}

/// The longest run of `items` from the front whose rendering fits under the ceiling, with what was left out
/// named in the caller's terms. `render` is handed a run and returns exactly what the verb would print for it,
/// so the ceiling holds the answer itself, separators and all. Where even one item is over the ceiling nothing
/// is kept: half an item is not an item.
pub fn cap_items<'a, T>(items: &'a [T], render: impl Fn(&[T]) -> String, said: Said<'_>) -> Capped<'a, T> {
  if chars(&render(items)) <= said.limit {
    return Capped {
      kept: items,
      left: 0,
      note: None,
    };
  }

  // The largest run that fits, found by halving: a longer run is a longer answer for every rendering these
  // verbs have.
  let mut fits = 0;
  let mut over = items.len();

  while over - fits > 1 {
    let middle = (fits + over) / 2;

    if chars(&render(&items[..middle])) <= said.limit {
      fits = middle;
    } else {
      over = middle;
    }
  }

  let left = items.len() - fits;
  let limit = count_text(said.limit);
  let note = if fits == 0 {
    format!(
      "-- one of the {} alone is over the {limit}-character ceiling, so nothing was given. {}",
      said.unit, said.how,
    )
  } else {
    format!(
      "-- cut at the {limit}-character ceiling: {} of {} {}. For the other {}: {}",
      count_text(fits),
      count_text(items.len()),
      said.unit,
      count_text(left),
      said.how,
    )
  };

  Capped {
    kept: &items[..fits],
    left,
    note: Some(note),
  }
}

/// One long value cut where it stands, with how much was cut said in place of the rest.
pub fn cap_value(text: &str, limit: usize) -> String {
  let held = chars(text);

  if held <= limit {
    return text.to_owned();
  }

  format!("{}…(+{} chars)", text.chars().take(limit).collect::<String>(), count_text(held - limit))
}

/// An answer of a tool: its structured body, whose text stands beside it.
#[derive(Debug, PartialEq)]
pub struct Answer {
  pub structured: Value,
}

impl Answer {
  /// The answer as a tool hands it: the structured body.
  pub fn structured(body: Value) -> Self {
    Self { structured: body }
  }

  /// The answer's text, the body written out whole.
  pub fn text(&self) -> String {
    serde_json::to_string(&self.structured).unwrap_or_default()
  }
}

/// The output schema every reading tool declares: the reading, one line each.
pub fn schema() -> Value {
  json!({
    "type": "object",
    "properties": {
      "lines": {
        "type": "array",
        "items": { "type": "string" },
        "description": "The reading, one line each.",
      },
    },
    "required": ["lines"],
  })
}

/// The lines as a reading tool answers them, `how` naming the argument that asks for the rest.
pub fn answer(lines: &[String], how: &str) -> Answer {
  let mut cut = Said::new("lines", how);

  // The object is written out whole as the text, so its quoting has room beside the lines.
  cut.limit = MAX_ANSWER_CHARS - 200;

  let capped = cap_items(lines, |kept| json!({ "lines": kept }).to_string(), cut);
  let mut kept: Vec<String> = capped.kept.to_vec();

  kept.extend(capped.note);

  Answer::structured(json!({ "lines": kept }))
}

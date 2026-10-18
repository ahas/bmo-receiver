//! Who drives the receiver's news, said only when somebody is driving it hard. The news writes nothing
//! and is checked against nothing, so anything holding the token can send it as fast as it can open
//! sockets; a line per call would bury the journal, so the calls are quiet until twenty land inside
//! ten seconds, and then one line names who sent them and the count starts again.

use std::sync::Mutex;

// What a caller says of itself is cut to what one journal line can carry.
pub const WHO_LIMIT: usize = 60;
const WINDOW_MS: i64 = 10_000;
const THRESHOLD: usize = 20;

/// What a relay says about itself, flattened to one line, since it is the caller's own unverified word
/// and a newline in it would forge a record of its own. A caller that says nothing is `unnamed`.
pub fn relay_caller(said: Option<&str>) -> String {
  let flat = said.unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ");

  if flat.is_empty() {
    "unnamed".to_owned()
  } else {
    flat.chars().take(WHO_LIMIT).collect()
  }
}

#[derive(Default)]
pub struct RelayBurst {
  seen: Mutex<Vec<(i64, String)>>,
}

impl RelayBurst {
  /// One relay call; the line to write where it completed a burst.
  pub fn saw(&self, who: &str, at: i64) -> Option<String> {
    let mut seen = self.seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner);

    seen.retain(|(when, _)| at - when < WINDOW_MS);
    seen.push((at, who.to_owned()));

    if seen.len() < THRESHOLD {
      return None;
    }

    let span = (at - seen[0].0) as f64 / 1000.0;
    let shown = format!("{:.1}", span);
    let line = format!("relay: {} events in {}s from {}", seen.len(), shown, names_of(&seen));

    seen.clear();

    Some(line)
  }
}

// Who sent them, commonest first and otherwise in the order they first came, with a count against any
// name that sent more than one.
fn names_of(seen: &[(i64, String)]) -> String {
  let mut tally: Vec<(&str, usize)> = Vec::new();

  for (_, who) in seen {
    match tally.iter_mut().find(|(name, _)| name == who) {
      Some((_, times)) => *times += 1,
      None => tally.push((who, 1)),
    }
  }

  tally.sort_by_key(|(_, times)| std::cmp::Reverse(*times));
  tally
    .iter()
    .map(|(who, times)| if *times == 1 { (*who).to_owned() } else { format!("{who} x{times}") })
    .collect::<Vec<_>>()
    .join(", ")
}

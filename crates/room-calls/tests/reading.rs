//! A reading tool's answer: its lines under the ceiling, as its object and its text alike (docs/chat-mcp.md,
//! 「The catalogue」).

use room_calls::reading::{MAX_ANSWER_CHARS, answer};
use serde_json::json;

#[test]
fn the_lines_stand_as_the_object_and_its_text_alike() {
  let answered = answer(&["one".to_owned(), "two".to_owned()], "Ask again narrowed.");

  assert_eq!(answered.structured, json!({ "lines": ["one", "two"] }));
  assert_eq!(answered.text(), json!({ "lines": ["one", "two"] }).to_string());
}

#[test]
fn a_reading_past_the_ceiling_keeps_whole_lines_and_says_how_to_ask_for_the_rest() {
  let lines: Vec<String> = (0..2_000).map(|at| format!("line {at} {}", "x".repeat(40))).collect();
  let answered = answer(&lines, "Ask again narrowed.");
  let kept = answered.structured["lines"].as_array().unwrap().clone();

  assert!(answered.text().chars().count() <= MAX_ANSWER_CHARS);
  assert!(kept.len() < lines.len());
  assert!(kept.last().unwrap().as_str().unwrap().contains("Ask again narrowed."));
}

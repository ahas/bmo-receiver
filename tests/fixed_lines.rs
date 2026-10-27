//! A fixed line where the web unit's language packs cannot render it (docs/voiced-notices.md, 「Lines that stay
//! fixed」): every line has a plain sentence in each language the policy holds, a line falls back to it with its
//! fields filled in as words, and a code with no sentence is dropped, never sent as its code and fields.

use bmo_receiver::receiver::speech::plain;
use receiver_codes::FixedLine;
use receiver_codes::draws::{DrawAsked, DrawLine, DrawResult};
use receiver_policy::fixed_lines::FIXED_LINES;
use serde_json::{Map, Value, json};
use strum::VariantNames;

fn fields_of(line: &FixedLine) -> (String, Map<String, Value>) {
  let Value::Object(mut fields) = serde_json::to_value(line).unwrap() else {
    panic!("a fixed line is an object");
  };
  let code = fields.remove("code").and_then(|code| code.as_str().map(str::to_owned)).unwrap();

  (code, fields)
}

#[test]
fn every_fixed_line_has_a_plain_sentence_in_every_language_the_policy_holds() {
  assert!(FIXED_LINES.sentences.contains_key("ko") && FIXED_LINES.sentences.contains_key("en"));

  for language in FIXED_LINES.sentences.keys() {
    for code in FixedLine::VARIANTS {
      assert!(FIXED_LINES.sentence(language, code).is_some(), "{code} has no sentence in {language}");
    }
  }
}

#[test]
fn a_line_falls_back_to_its_sentence_with_its_fields_as_words_and_never_as_its_code() {
  let draw = FixedLine::Draw(DrawLine {
    label: None,
    asked: DrawAsked::Dice {
      count: 2,
      sides: 6,
      modifier: 0,
    },
    result: DrawResult::Dice {
      faces: vec![3, 5],
      total: 8,
    },
    number: 4,
    seed: "ab".to_owned(),
    method: "bmo-draw-v1".to_owned(),
    answered: None,
    next: "cd".to_owned(),
  });

  for line in [
    draw,
    FixedLine::RunBroken {
      agent: "agent-a".to_owned(),
      cause: None,
      record: None,
    },
    FixedLine::GoingDownForUpdate {
      items: vec![12, 14],
    },
  ] {
    let (code, fields) = fields_of(&line);

    for language in ["ko", "en"] {
      let said =
        plain(language, &code, &fields).unwrap_or_else(|| panic!("{code} in {language} says nothing"));

      assert!(!said.contains(&code) && !said.contains('{') && !said.contains('"'), "{code}: {said}");
    }
  }

  let (code, fields) = fields_of(&FixedLine::Draw(DrawLine {
    label: Some("who goes first".to_owned()),
    asked: DrawAsked::Pick { items: 3, take: 1 },
    result: DrawResult::Pick {
      picked: vec!["agent-b".to_owned()],
    },
    number: 9,
    seed: "ef".to_owned(),
    method: "bmo-draw-v1".to_owned(),
    answered: None,
    next: "01".to_owned(),
  }));
  let said = plain("en", &code, &fields).unwrap();

  assert!(said.contains("Draw 9") && said.contains("agent-b") && said.contains("ef"), "{said}");
}

#[test]
fn a_code_no_sentence_can_be_made_of_is_dropped() {
  assert_eq!(plain("ko", "no-such-line", &Map::new()), None);
  assert_eq!(plain("xx", "going-down", &Map::new()), None);

  let mut fields = Map::new();

  fields.insert("agent".to_owned(), json!("agent-a"));
  assert_eq!(
    plain("en", "run-broken", &fields).as_deref(),
    Some("The work of agent-a stopped partway."),
    "an empty field leaves no gap"
  );
}

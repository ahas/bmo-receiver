//! The fair draw's own arithmetic: the stream, the published vector, a request read, a spec kept and a
//! published draw checked (docs/random-draw.md).

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use bmo_receiver::verbs::draws::{
  Claim, Claimed, Outcome, Spec, Stream, commitment_of, draw_with, hex, read_request, result_text,
  verify_published,
};

fn spec(json: &str) -> Map<String, Value> {
  serde_json::from_str(json).unwrap()
}

#[test]
fn the_stream_is_sha256_of_seed_number_and_block_read_as_big_endian_words() {
  let seed = "0".repeat(64);
  let mut stream = Stream::new(&seed, 7);
  let block = Sha256::digest(format!("{seed}:7:0").as_bytes());

  assert_eq!(stream.word(), u32::from_be_bytes(block[0..4].try_into().unwrap()));
  assert_eq!(stream.word(), u32::from_be_bytes(block[4..8].try_into().unwrap()));
}

// The test vector docs/random-draw.md publishes, which the draw is checked against here too.
#[test]
fn the_published_vector_draws_what_the_document_says() {
  let seed = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
  let items = |names: &[&str]| names.iter().map(|name| name.to_string()).collect::<Vec<String>>();

  assert_eq!(commitment_of(seed), "6c86c6aac5fb24bcf5d9939cb7d7d5645ce39418f449e03b262dd4fa14b4b92b");
  assert_eq!(
    hex(&Sha256::digest(format!("{seed}:1:0").as_bytes())),
    "f35dfdb8f3485c8047e663e36e0b4a13fd85b02df5dd85b40bf17e9f727301a9",
  );
  assert_eq!(
    draw_with(
      &Spec::Dice {
        sides: 6,
        count: 3,
        modifier: 2
      },
      seed,
      1
    ),
    Outcome::Dice {
      faces: vec![3, 5, 4],
      total: 14
    },
  );
  assert_eq!(draw_with(&Spec::Range { min: 1, max: 100 }, seed, 1), Outcome::Range { value: 89 });
  assert_eq!(
    draw_with(
      &Spec::Pick {
        items: items(&["A", "B", "C", "D", "E"]),
        take: 2
      },
      seed,
      1
    ),
    Outcome::Pick {
      picked: items(&["C", "B"])
    },
  );
  assert_eq!(
    draw_with(
      &Spec::Shuffle {
        items: items(&["A", "B", "C", "D", "E"])
      },
      seed,
      1
    ),
    Outcome::Shuffle {
      order: items(&["C", "B", "E", "A", "D"])
    },
  );
}

#[test]
fn a_request_is_read_with_its_defaults_and_a_stray_field_is_refused() {
  assert_eq!(
    read_request(&spec(r#"{"shape":"dice","sides":6}"#)).unwrap().spec,
    Spec::Dice {
      sides: 6,
      count: 1,
      modifier: 0
    },
  );
  assert_eq!(
    read_request(&spec(r#"{"shape":"dice","sides":6,"items":["a"]}"#)).unwrap_err(),
    "items does not belong to a dice draw",
  );
  assert_eq!(
    read_request(&spec(r#"{"shape":"coin"}"#)).unwrap_err(),
    "shape must be one of dice, range, pick, shuffle",
  );
  assert_eq!(
    read_request(&spec(r#"{"shape":"range","min":3,"max":1}"#)).unwrap_err(),
    "min must not be above max",
  );
  assert_eq!(
    read_request(&spec(r#"{"shape":"dice","sides":1.5}"#)).unwrap_err(),
    "sides must be a whole number",
  );
}

#[test]
fn a_spec_is_kept_in_the_order_the_tool_keeps_it() {
  assert_eq!(
    serde_json::to_string(&Spec::Dice {
      sides: 6,
      count: 2,
      modifier: 1
    })
    .unwrap(),
    r#"{"shape":"dice","sides":6,"count":2,"modifier":1}"#,
  );
  assert_eq!(
    serde_json::to_string(&Outcome::Dice {
      faces: vec![3, 4],
      total: 8
    })
    .unwrap(),
    r#"{"shape":"dice","faces":[3,4],"total":8}"#,
  );
}

#[test]
fn a_published_draw_is_checked_by_its_commitment_and_its_result() {
  let seed = "ab".repeat(32);
  let args = spec(r#"{"shape":"range","min":1,"max":100}"#);
  let drawn = draw_with(&read_request(&args).unwrap().spec, &seed, 3);
  let claim = |commitment: Option<String>, result: Option<String>| Claim {
    seed: seed.clone(),
    draw_id: 3,
    spec: args.clone(),
    method: None,
    commitment,
    result: result.map(Claimed::Text),
  };

  assert!(verify_published(&claim(Some(commitment_of(&seed)), Some(result_text(&drawn)))).matched);
  assert!(!verify_published(&claim(Some("00".repeat(32)), None)).matched);
  assert!(!verify_published(&claim(None, None)).matched);
}

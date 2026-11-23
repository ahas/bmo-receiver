//! The secret shapes as the receiver reads a message's words for them (docs/messages.md, 「Secrets taken
//! out」): a run of a shape the base library holds is replaced by the receiver's marker, every string of a
//! payload is read and its keys and numbers stand, and the marker is read as no word. Each secret-shaped value is
//! assembled at run time, so no tracked file holds one (docs/PERSONAL-DATA.md). The shapes themselves are held to
//! their runs by the library's own tests.

use bmo_receiver::secrets::{marker, only_marked, redact, redact_value, redacted, unmarked};
use serde_json::json;

#[test]
fn a_run_of_a_shape_is_replaced_by_the_receiver_s_marker() {
  let key = format!("{}{}", "sk", "-or-v1-".to_owned() + &"0123456789abcdef".repeat(4));
  let read = redact(&format!("use {key} for the road, thanks"));

  assert_eq!(read.text, format!("use {} for the road, thanks", marker()));
  assert_eq!(read.runs, 1);
  assert_eq!(
    redacted("a plain sentence with nothing to hide"),
    "a plain sentence with nothing to hide"
  );
}

#[test]
fn the_marker_is_no_word() {
  let only = format!("  {0}\n{0} ", marker());

  assert!(only_marked(&only));
  assert!(!only_marked(&format!("{} here it is", marker())));
  assert!(!only_marked("   "));
  assert!(!unmarked(&format!("a{}b", marker())).contains(marker()));
}

#[test]
fn every_string_of_a_payload_is_read_and_its_keys_and_numbers_stand() {
  let token = "0123456789abcdef".repeat(4);
  let mut payload = json!({
    "text": format!("the token is {token}"),
    "entities": [{ "type": "code", "offset": 13, "length": 64, "note": token }],
    "id": 7,
  });

  assert_eq!(redact_value(&mut payload), 2);
  assert_eq!(
    payload,
    json!({
      "text": format!("the token is {}", marker()),
      "entities": [{ "type": "code", "offset": 13, "length": 64, "note": marker() }],
      "id": 7,
    })
  );
}

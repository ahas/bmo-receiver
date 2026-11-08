//! The rooms tool: its entries and the schemas its manifest names.

use std::path::Path;

use rooms_tool::ENTRIES;

#[test]
fn the_entries_the_manifest_names_are_answered_and_their_schemas_stand_beside() {
  for name in ["room_open", "room_list", "room_supervisors"] {
    assert!(ENTRIES.iter().any(|entry| entry.name == name), "{name} is answered");
  }

  for entry in ENTRIES {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.input);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|why| panic!("{path:?}: {why}"));
    let schema: serde_json::Value =
      serde_json::from_str(&text).unwrap_or_else(|why| panic!("{path:?}: {why}"));

    assert_eq!(schema["type"], "object", "{path:?}");
    assert_eq!(schema["additionalProperties"], false, "{path:?}");
  }
}

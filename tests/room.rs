//! The room block's own readings: the names one agent answers to, the order they are compared in, the
//! capability sections a road is given.

use bmo_receiver::chat::room::{Trees, capability_context, name_order, names_of};
use bmo_receiver::roads::declared::Facts;

#[test]
fn a_name_list_opens_on_the_owners_word() {
  assert_eq!(
    names_of("worker", Some("worker label"), Some("a, worker label, b")),
    "worker label, a, b"
  );
  assert_eq!(names_of("bare", None, None), "bare");
}

#[test]
fn names_compare_with_case_set_aside_first() {
  let mut names = vec!["b", "A", "a", "tester-w2", "tester-w10"];

  names.sort_by(|left, right| name_order(left, right));

  assert_eq!(names, vec!["a", "A", "b", "tester-w10", "tester-w2"]);
}

// A road declaring the verbs given, its dialect folding or not.
fn road(can: &[&str], foldable: bool) -> Facts {
  Facts {
    key: "road-a".to_owned(),
    label: "road a".to_owned(),
    can: can.iter().map(|verb| (*verb).to_owned()).collect(),
    unpinned: vec![],
    writing: String::new(),
    foldable,
    shows: None,
    speakers: false,
    file_bytes: None,
  }
}

#[test]
fn the_fold_section_stands_only_where_the_road_s_dialect_folds() {
  let data = tempfile::tempdir().unwrap();
  let trees = Trees {
    data: data.path().to_path_buf(),
  };
  let chat = trees.agent_dir("chat");

  std::fs::create_dir_all(&chat).unwrap();
  std::fs::write(
    chat.join("capabilities.md"),
    "OPENING\n\nCAPABILITY replyTo: pinning\n- the reply rule\n\nCAPABILITY foldable: folding\n- the fold rule\n",
  )
  .unwrap();

  let folding = capability_context(&trees, &road(&["text", "replyTo"], true), None).unwrap();

  assert!(folding.contains("the fold rule"), "{folding}");
  assert!(folding.contains("the reply rule"), "{folding}");

  let flat = capability_context(&trees, &road(&["text", "replyTo"], false), None).unwrap();

  assert!(!flat.contains("the fold rule"), "{flat}");
  assert!(flat.contains(": folding."), "the fold is named among what the road does not do: {flat}");

  // The fold is no verb the room's own road carries for another: a road that does not fold is never handed it.
  let home = road(&["text"], true);
  let away = capability_context(&trees, &road(&["text"], false), Some(&home)).unwrap();

  assert!(!away.contains("the fold rule"), "{away}");
  assert!(!away.contains("sends these into this same room"), "{away}");
}

// A road that carries the sending verbs is told no sending list: every entry is shown to every room, and the
// road judges each send itself. A block that says none while the road sends is the disagreement this holds off.
#[test]
fn a_road_that_sends_is_not_told_it_can_send_none() {
  let data = tempfile::tempdir().unwrap();
  let trees = Trees {
    data: data.path().to_path_buf(),
  };
  let chat = trees.agent_dir("chat");

  std::fs::create_dir_all(&chat).unwrap();
  std::fs::write(
    chat.join("capabilities.md"),
    "OPENING\n\nCAPABILITY replyTo: pinning\n- the reply rule\n\nCAPABILITY foldable: folding\n- the fold rule\n",
  )
  .unwrap();

  // The verbs a messenger such as Telegram declares, the ones photo, file, voice and the rest ask for.
  let sends = road(&["text", "album", "document", "voice", "sticker", "reaction", "pin", "replyTo"], true);
  let block = capability_context(&trees, &sends, None).unwrap();

  assert!(!block.contains("SENDING TOOLS ON THIS ROAD"), "{block}");
  assert!(!block.contains("— none"), "{block}");
  assert!(block.contains("the reply rule") && block.contains("the fold rule"), "{block}");
}

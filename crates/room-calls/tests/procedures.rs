//! A procedure document as one room may use it: the sections and lines for a verb that room's road does not
//! perform left out (docs/chat-mcp.md, 「Procedures as resources」).

use room_calls::bot::Service;
use room_calls::procedures::{
  as_used, filter_procedure, procedure_names, procedures_dir, road_can, written,
};
use tempfile::tempdir;

const DOCUMENT: &str = "# Sending\n\nEvery room reads this.\n\n## Stickers [CAP:sticker]\n\n- send one\n\n## Lines\n\n- a line [CAP:text]\n- a pin [CAP:pin]\n  - its fine print\n- plain\n";

fn verbs(names: &[&str]) -> Vec<String> {
  names.iter().map(|name| name.to_string()).collect()
}

// A tree whose procedures folder holds the document under the name `sending`.
fn tree() -> tempfile::TempDir {
  let placed = tempdir().unwrap();

  std::fs::create_dir_all(procedures_dir(placed.path())).unwrap();
  std::fs::write(procedures_dir(placed.path()).join("sending.md"), DOCUMENT).unwrap();
  std::fs::write(procedures_dir(placed.path()).join("notes.txt"), "not a procedure").unwrap();
  placed
}

#[test]
fn a_room_that_performs_every_verb_reads_the_document_whole_minus_the_marks() {
  let read = filter_procedure(DOCUMENT, &verbs(&["sticker", "text", "pin"])).unwrap();

  assert_eq!(
    read,
    DOCUMENT
      .replace(" [CAP:sticker]", "")
      .replace(" [CAP:text]", "")
      .replace(" [CAP:pin]", ""),
  );
}

#[test]
fn a_mark_on_a_line_takes_that_line_alone_and_a_heading_its_section() {
  let read = filter_procedure(DOCUMENT, &verbs(&["text"])).unwrap();

  assert_eq!(read, "# Sending\n\nEvery room reads this.\n\n## Lines\n\n- a line\n- plain\n");
}

#[test]
fn a_mark_naming_something_that_is_no_capability_is_a_broken_document() {
  let broken = filter_procedure("- a line [CAP:shout]\n", &verbs(&["text"])).unwrap_err();

  assert_eq!(broken, "a procedure marks \"shout\", which is no capability");
}

#[test]
fn the_procedures_are_the_markdown_files_of_the_folder_read_whole_outside_a_room_and_filtered_in_one() {
  let placed = tree();
  let text = written(placed.path(), "sending").unwrap();

  assert_eq!(procedure_names(placed.path()), ["sending"]);
  assert_eq!(as_used(&text, None).unwrap(), DOCUMENT.trim_end());
  assert_eq!(
    as_used(&text, Some(&verbs(&["text"]))).unwrap(),
    "# Sending\n\nEvery room reads this.\n\n## Lines\n\n- a line\n- plain",
  );
  assert_eq!(
    written(placed.path(), "notes").unwrap_err(),
    "no procedure named \"notes\"; there are: sending"
  );
}

#[tokio::test]
async fn a_receiver_with_no_token_is_not_asked_for_a_roads_verbs() {
  assert!(road_can(&Service::default(), -1, None).await.unwrap_err().contains("RECEIVER_TOKEN"));
}

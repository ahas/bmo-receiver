//! The open-file watch (docs/receiver.md, 「The open-file ceiling」): one line as the count reaches its share of the
//! ceiling, with the count and the ceiling, one as it falls back under, and none while it stays on its side.

use bmo_receiver::open_files::{Watch, ceiling, held};

#[test]
fn the_watch_says_each_crossing_of_its_share_once() {
  let mut watch = Watch::new(80);

  assert_eq!(watch.read(100, 1024), None, "a count far under the share says nothing");

  let near = watch.read(820, 1024).expect("reaching the share is said");

  assert!(near.contains("820") && near.contains("1024"), "{near}");
  assert_eq!(watch.read(1000, 1024), None, "a count still near says nothing more");
  assert_eq!(watch.read(1024, 1024), None, "nor does one at the ceiling");

  let back = watch.read(500, 1024).expect("falling back under is said");

  assert!(back.contains("500") && back.contains("1024"), "{back}");
  assert_eq!(watch.read(400, 1024), None);
  assert!(watch.read(900, 1024).is_some(), "a second crossing is said again");
}

#[test]
fn the_process_s_open_files_and_its_ceiling_are_read() {
  let open = held().expect("the system lists the process's open files");
  let ceiling = ceiling().expect("the system bounds them");

  assert!(open > 0 && open < ceiling, "{open} of {ceiling}");
}

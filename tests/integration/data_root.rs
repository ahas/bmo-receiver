//! The stores under the data root that keep a folder per room, and the spool emptied as the service starts
//! (`docs/store.md`, 「The data root's folders」).

use std::path::Path;

use bmo_receiver::data_root::DataRoot;

use crate::integration::common::alone::alone;

#[test]
fn a_rooms_folders_are_found_only_where_they_stand() {
  let Some(_run) = alone(module_path!(), "a_rooms_folders_are_found_only_where_they_stand") else {
    return;
  };

  // SAFETY: this process runs this one test alone, so no other thread reads the environment.
  unsafe {
    std::env::set_var("BMO_COLLECT_DIR", "");
    std::env::remove_var("BMO_IMAGE_DIR");
  }

  let dir = tempfile::tempdir().unwrap();
  let root = DataRoot::new(dir.path());

  assert_eq!(root.collected(), dir.path().join("collected"), "a variable set empty names no folder");
  assert_eq!(root.outbox(-7), dir.path().join("outbox").join("-7"));
  assert!(root.room_folders(-7).is_empty());
  assert!(!dir.path().join("outbox").exists(), "finding a folder makes nothing");

  std::fs::create_dir_all(dir.path().join("collected/-7")).unwrap();
  std::fs::create_dir_all(dir.path().join("outbox/-7")).unwrap();
  std::fs::create_dir_all(dir.path().join("images/-8")).unwrap();

  assert_eq!(root.room_folders(-7), [dir.path().join("collected/-7"), dir.path().join("outbox/-7")]);
}

#[test]
fn a_variable_moves_the_message_file_and_picture_stores() {
  let Some(_run) = alone(module_path!(), "a_variable_moves_the_message_file_and_picture_stores") else {
    return;
  };

  let elsewhere = tempfile::tempdir().unwrap();
  let collected = elsewhere.path().join("kept");
  let images = elsewhere.path().join("pictures");

  // SAFETY: this process runs this one test alone, so no other thread reads the environment.
  unsafe {
    std::env::set_var("BMO_COLLECT_DIR", &collected);
    std::env::set_var("BMO_IMAGE_DIR", &images);
  }

  let root = DataRoot::new(Path::new("/nowhere"));

  assert_eq!(root.collected(), collected);
  assert_eq!(root.images(), images);
  assert_eq!(
    root.outboxes(),
    Path::new("/nowhere/outbox"),
    "the outboxes stand where the data root is"
  );

  std::fs::create_dir_all(images.join("-7")).unwrap();
  assert_eq!(root.room_folders(-7), [images.join("-7")]);
}

#[test]
fn the_start_empties_the_spool_and_leaves_the_rooms_files() {
  let dir = tempfile::tempdir().unwrap();
  let root = DataRoot::new(dir.path());
  let left = root.spool().join("part-1");
  let kept = root.collected().join("-7/3-notes.txt");

  std::fs::create_dir_all(root.spool()).unwrap();
  std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
  std::fs::write(&left, b"left by a stopped receiver").unwrap();
  std::fs::write(&kept, b"kept").unwrap();

  bmo_receiver::service_start::emptied_spool(&root);

  assert!(!left.exists(), "a part no holder will let go of is removed");
  assert!(root.spool().is_dir(), "the spool itself stands");
  assert_eq!(std::fs::read(&kept).unwrap(), b"kept");
}

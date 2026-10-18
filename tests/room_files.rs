//! What a room keeps stays in that room (docs/room-files.md).

use tempfile::tempdir;

use bmo_receiver::verbs::room_files::room_file_refusal;

#[test]
fn a_rooms_file_goes_back_into_that_room_alone_and_never_out_of_a_job() {
  let data = tempdir().unwrap();
  let stores = vec![data.path().join("collected")];
  let kept = data.path().join("collected").join("-100").join("7-report.pdf");

  std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
  std::fs::write(&kept, b"pdf").unwrap();

  let path = kept.to_string_lossy().into_owned();

  assert_eq!(room_file_refusal(&path, -100, &stores, false), None);
  assert!(room_file_refusal(&path, 42, &stores, false).unwrap().contains("is kept for room -100"));
  assert!(
    room_file_refusal(&path, -100, &stores, true)
      .unwrap()
      .contains("room -100 keeps, and a job sends none")
  );
  assert_eq!(room_file_refusal("/tmp/elsewhere.png", 42, &stores, true), None);
}

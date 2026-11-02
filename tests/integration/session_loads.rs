//! What a session holds of what was sent in full, kept in the agency's record of its loads (docs/unified-bmo.md,
//! 「The system prompt and the room switch」; docs/boundary-api.md, 「The agency」): a keep replaces the whole
//! list, so the turn's loads are laid over the ones the session held, by path, and another session's stay its own.

use crate::integration::common::Scratch;
use crate::integration::common::units::standing;
use bmo_receiver::chat::context::{fingerprint, remember_held};
use bmo_receiver::chat::room::RoomContext;
use bmo_receiver::data_root::DataRoot;
use receiver_ports::agency::Load;
use tempfile::tempdir;

const ROOM: i64 = -7;

fn load(path: &str, version: &str) -> Load {
  Load {
    path: path.to_owned(),
    version: version.to_owned(),
  }
}

#[tokio::test]
async fn a_turns_loads_are_laid_over_the_ones_its_session_held_and_another_sessions_stay() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);
  let room = RoomContext {
    chat_id: ROOM,
    text: "the room's block".to_owned(),
    mark: "m".to_owned(),
  };
  let block = load(&format!("room-context/{ROOM}"), &fingerprint(&room));

  standing.agency.loads.lock().unwrap().extend([
    ("s-1".to_owned(), vec![load("docs/a.md", "v1"), load(&block.path, "older")]),
    ("s-2".to_owned(), vec![load("docs/b.md", "v2")]),
  ]);

  remember_held(&standing.receiver, Some("s-1"), "s-1", Some(&room), true, "", None)
    .await
    .unwrap();

  let loaded = standing.agency.loaded();

  assert_eq!(
    loaded["s-1"],
    [load("docs/a.md", "v1"), block.clone()],
    "the room's new version replaces its old"
  );
  assert_eq!(loaded["s-2"], [load("docs/b.md", "v2")], "another session's loads are not touched");

  // A session that answered under a new id keeps what it held itself, not what the resumed one held.
  let changed = RoomContext {
    text: "the room's block, changed".to_owned(),
    ..room
  };

  remember_held(&standing.receiver, Some("s-1"), "s-2", Some(&changed), true, "", None)
    .await
    .unwrap();

  let loaded = standing.agency.loaded();

  assert_eq!(loaded["s-2"], [load("docs/b.md", "v2"), load(&block.path, &fingerprint(&changed))]);
  assert_eq!(loaded["s-1"], [load("docs/a.md", "v1"), block]);
}

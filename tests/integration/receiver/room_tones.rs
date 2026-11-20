//! A room's tone layer (docs/receiver.md, 「A room's tone layer」; docs/api.md, 「Rooms」): a room stands on none
//! until the administrator sets it to a layer the deployment offers, anyone else and a layer it does not offer
//! are refused, and the room's block carries the layer it is set to and nothing once it is set to none.

use std::sync::Arc;

use bmo_receiver::chat::room::room_context;
use receiver_policy::chat_line::CHAT_LINE;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, post, put, stood_on};
use crate::integration::common::units::{AGENT, OWNER, StandInChat, TONES};

const ROOM: i64 = -100;
const ADMIN: &str = "the-admin";
const LAYER: &str = "Keep it short with strangers.";

// A front reaching one chat service that holds the room, the web unit answering for one administrator, its data
// root holding the chat's documents and the first offered layer's file.
async fn front(scratch: &Scratch) -> Front {
  let service = Arc::new(StandInChat::new("road-a").holding(&[ROOM]));
  let front = stood_on(scratch, OWNER, vec![service]).await;
  let prompts = front.data.path().join("prompts");

  front.standing.web.admin(ADMIN.to_owned());
  std::fs::create_dir_all(prompts.join("chat")).unwrap();
  std::fs::create_dir_all(prompts.join("persona")).unwrap();
  std::fs::write(prompts.join("chat").join(&CHAT_LINE.room.owner_document), "The owner's own chat.")
    .unwrap();
  std::fs::write(prompts.join("chat").join(&CHAT_LINE.room.group_document), "A group room.").unwrap();
  std::fs::write(prompts.join("chat").join(&CHAT_LINE.capabilities.document), "What the agent can do.")
    .unwrap();
  std::fs::write(prompts.join("persona").join(format!("{}.md", TONES[0])), LAYER).unwrap();

  let (status, room) =
    post(&front, "/rooms", json!({ "provider": "road-a", "roomId": ROOM, "mode": "record-only" })).await;

  assert_eq!(status.as_u16(), 201, "{room}");
  front
}

// The room's block as its next turn would carry it.
async fn block(front: &Front) -> String {
  room_context(&front.receiver(), AGENT, ROOM, None).await.unwrap().text
}

#[tokio::test]
async fn the_administrator_sets_a_rooms_tone_layer_and_its_block_carries_it() {
  let scratch = Scratch::new().await;
  let front = front(&scratch).await;
  let path = format!("/rooms/{ROOM}/tone");
  let as_admin = format!("{path}?by={ADMIN}");

  let (status, standing) = get(&front, &path).await;

  assert_eq!(status.as_u16(), 200, "{standing}");
  assert_eq!(standing, json!({ "room": ROOM, "tone": null, "tones": TONES }));
  assert!(
    !block(&front).await.contains(&CHAT_LINE.tone.header),
    "a room set to none carries no layer"
  );

  // Anyone but the administrator, a layer the deployment does not offer and a body naming no tone are refused.
  let (status, refused) =
    put(&front, &format!("{path}?by=someone-else"), json!({ "tone": TONES[0] })).await;

  assert_eq!(
    (status.as_u16(), refused["code"].as_str()),
    (403, Some("not-administrator")),
    "{refused}"
  );

  let (status, refused) = put(&front, &as_admin, json!({ "tone": "unoffered" })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (400, Some("unknown-tone")), "{refused}");

  let (status, refused) = put(&front, &as_admin, json!({})).await;

  assert_eq!(status.as_u16(), 400, "{refused}");
  assert_eq!(get(&front, &path).await.1["tone"], json!(null), "nothing refused was set");

  // Asked as the console asks it, naming the administrator's account beside the tone.
  let (status, set) = put(&front, &as_admin, json!({ "tone": TONES[0], "account": 1 })).await;

  assert_eq!(status.as_u16(), 200, "{set}");
  assert_eq!(set["tone"], TONES[0]);
  assert!(block(&front).await.contains(&format!("{}\n{LAYER}", CHAT_LINE.tone.header)));

  let (status, cleared) = put(&front, &as_admin, json!({ "tone": null })).await;

  assert_eq!(status.as_u16(), 200, "{cleared}");
  assert_eq!(cleared["tone"], json!(null));
  assert!(!block(&front).await.contains(LAYER), "a room set back to none carries no layer");
}

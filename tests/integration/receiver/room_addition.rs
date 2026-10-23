//! A room added on a chat service in a mode (docs/receiver.md, 「Adding a room」; docs/api.md, 「Rooms」): what each
//! provider offers read off its declaration, the rooms of its account relayed page by page, a room kept with its
//! mode, its title and its providers, every refusal where the service does not declare what the choice needs, and
//! a room answered as the user sending in the account of the person the service reads.

use std::sync::Arc;

use bmo_chat_client::{ChatCapability, ChatModeStanding, ChatSend, ChatSendRequest, ChatVoice};
use serde_json::{Value, json};

use crate::common::Scratch;
use crate::common::queue::{Front, get, post, stood_on};
use crate::common::units::{OWNER, StandInChat};

// A service that lists its account's rooms and answers as the person it reads, and one that does neither.
async fn front(scratch: &Scratch) -> (Front, Arc<StandInChat>, Arc<StandInChat>) {
  let listing = Arc::new(
    StandInChat::new("road-a")
      .holding(&[-300, -200, -100])
      .titling(-200, "Second Room")
      .canning(&[ChatCapability::RoomList]),
  );
  let plain = Arc::new(StandInChat::new("road-b").holding(&[-400]));

  listing.moding(ChatVoice::BotAccount, ChatModeStanding::Active);
  listing.moding(ChatVoice::HumanAvatar, ChatModeStanding::Active);

  let front = stood_on(scratch, OWNER, vec![listing.clone(), plain.clone()]).await;

  (front, listing, plain)
}

fn adding(provider: &str, room: i64, mode: &str) -> Value {
  json!({ "provider": provider, "roomId": room, "mode": mode })
}

#[tokio::test]
async fn each_provider_offers_what_it_declares_and_the_record_offers_nothing() {
  let scratch = Scratch::new().await;
  let (front, listing, _plain) = front(&scratch).await;

  let (status, every) = get(&front, "/providers").await;

  assert_eq!(status.as_u16(), 200, "{every}");
  assert_eq!(
    every["items"],
    json!([
      { "key": "record", "label": every["items"][0]["label"], "listsRooms": false, "roomModes": [] },
      { "key": "road-a", "label": "the road-a road", "listsRooms": true, "roomModes": ["avatar", "record-only"] },
      { "key": "road-b", "label": "the road-b road", "listsRooms": false, "roomModes": ["record-only"] },
    ])
  );

  // A mode's standing moves while the service runs, and what is offered follows it.
  listing.moding(ChatVoice::HumanAvatar, ChatModeStanding::Unsupported);

  assert_eq!(get(&front, "/providers").await.1["items"][1]["roomModes"], json!(["record-only"]));
}

#[tokio::test]
async fn the_rooms_of_an_account_are_relayed_page_by_page_and_refused_where_the_service_lists_none() {
  let scratch = Scratch::new().await;
  let (front, _listing, _plain) = front(&scratch).await;

  let (status, page) = get(&front, "/providers/road-a/rooms?after=-300&limit=1").await;

  assert_eq!(status.as_u16(), 200, "{page}");
  assert_eq!(
    page,
    json!({
      "items": [{ "id": -200, "title": "Second Room", "kind": "group", "privateTo": null, "mode": null }],
      "more": true,
    })
  );

  for (path, status, code) in [
    ("/providers/road-b/rooms", 409, "rooms-unlisted"),
    ("/providers/record/rooms", 409, "rooms-unlisted"),
    ("/providers/road-z/rooms", 404, "not-found"),
    ("/providers/road-a/rooms?after=first", 400, "bad-request"),
  ] {
    let (answered, body) = get(&front, path).await;

    assert_eq!((answered.as_u16(), body["code"].as_str()), (status, Some(code)), "{path}: {body}");
  }
}

#[tokio::test]
async fn a_room_added_is_kept_with_its_mode_its_title_and_its_service_beside_the_record() {
  let scratch = Scratch::new().await;
  let (front, _listing, _plain) = front(&scratch).await;

  let (status, room) = post(&front, "/rooms", adding("road-a", -200, "record-only")).await;

  assert_eq!(status.as_u16(), 201, "{room}");
  assert_eq!((room["id"].as_i64(), room["mode"].as_str()), (Some(-200), Some("record-only")));
  assert_eq!(room["title"], "Second Room", "the service's title stands where none is given");
  assert_eq!(room["registered"], true);
  assert_eq!(
    get(&front, "/rooms/-200/providers").await.1,
    json!({ "providers": ["road-a", "record"], "chosen": true })
  );

  // The room stands in the room list before anything is said there, and the account's list shows its mode.
  let listed = get(&front, "/rooms").await.1;
  let shown = listed["items"].as_array().unwrap().iter().find(|one| one["id"] == -200).cloned();

  assert_eq!(shown.map(|one| one["mode"].clone()), Some(json!("record-only")), "{listed}");
  assert_eq!(
    get(&front, "/providers/road-a/rooms?after=-300&limit=1").await.1["items"][0]["mode"],
    "record-only"
  );

  // A title the caller names stands over the service's.
  let mut named = adding("road-a", -100, "avatar");

  named["title"] = json!("Named Room");

  let (status, room) = post(&front, "/rooms", named).await;

  assert_eq!(
    (status.as_u16(), room["title"].as_str(), room["mode"].as_str()),
    (201, Some("Named Room"), Some("avatar"))
  );
}

#[tokio::test]
async fn an_addition_the_service_does_not_declare_or_a_room_added_before_is_refused_and_writes_nothing() {
  let scratch = Scratch::new().await;
  let (front, listing, _plain) = front(&scratch).await;

  post(&front, "/rooms", adding("road-a", -300, "record-only")).await;

  for (body, status, code) in [
    (adding("road-b", -400, "avatar"), 409, "avatar-unsupported"),
    (adding("road-a", -999, "record-only"), 409, "room-unknown"),
    (adding("road-a", -300, "avatar"), 409, "room-added"),
    (adding("record", -500, "record-only"), 400, "bad-request"),
    (adding("road-z", -500, "record-only"), 400, "bad-request"),
    (adding("road-a", -100, "silent"), 400, "bad-request"),
    (json!({ "provider": "road-a", "mode": "avatar" }), 400, "bad-request"),
  ] {
    let (answered, refused) = post(&front, "/rooms", body.clone()).await;

    assert_eq!((answered.as_u16(), refused["code"].as_str()), (status, Some(code)), "{body}: {refused}");
  }

  // A service whose avatar mode stopped standing active offers the user alone.
  listing.moding(ChatVoice::HumanAvatar, ChatModeStanding::Inactive);

  let (answered, refused) = post(&front, "/rooms", adding("road-a", -100, "avatar")).await;

  assert_eq!(
    (answered.as_u16(), refused["code"].as_str()),
    (409, Some("avatar-unsupported")),
    "{refused}"
  );

  let kept = scratch
    .texts(r#"SELECT "chat_id"::text || ' ' || "mode" FROM "room_modes" ORDER BY "chat_id""#)
    .await;

  assert_eq!(kept, ["-300 record-only"], "nothing refused was written");
  assert_eq!(
    scratch.texts(r#"SELECT DISTINCT "chat_id"::text FROM "room_providers""#).await,
    ["-300"],
    "no refused room had its providers set"
  );
}

#[tokio::test]
async fn a_room_answered_as_the_user_sends_in_the_account_of_the_person_the_service_reads() {
  let scratch = Scratch::new().await;
  let (front, listing, _plain) = front(&scratch).await;
  let receiver = front.receiver();

  post(&front, "/rooms", adding("road-a", -100, "avatar")).await;
  post(&front, "/rooms", adding("road-a", -200, "record-only")).await;

  // A line the record keeps goes on to the service, and a send on the service's road goes out the same way.
  receiver.speech().kept(-100, "kept in the record", None).await.unwrap();
  receiver
    .speech()
    .act("road-a", &ChatSendRequest::new(-100, ChatSend::Typing { on: true }))
    .await
    .unwrap();

  // The owner's own words keep the owner's voice, and a room for the user alone keeps the service's.
  let mut owners = ChatSendRequest::new(-100, ChatSend::Typing { on: true });

  owners.mode = Some(ChatVoice::HumanReader);
  receiver.speech().act("road-a", &owners).await.unwrap();
  receiver.speech().kept(-200, "for the user alone", None).await.unwrap();

  let voices: Vec<(i64, Option<ChatVoice>)> =
    listing.sent().iter().map(|request| (request.room_id, request.mode)).collect();

  assert_eq!(
    voices,
    [
      (-100, Some(ChatVoice::HumanAvatar)),
      (-100, Some(ChatVoice::HumanAvatar)),
      (-100, Some(ChatVoice::HumanReader)),
      (-200, None),
    ]
  );
}

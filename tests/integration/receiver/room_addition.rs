//! A room added on a chat service in a mode (docs/receiver.md, 「Adding a room」; docs/api.md, 「Rooms」): what each
//! provider offers read off its declaration, the rooms of its account relayed page by page, a room kept with its
//! mode, its title and its providers, every refusal where the service does not declare what the choice needs, and
//! a room answered as the user sending in the account of the person the service reads.

use std::collections::BTreeSet;
use std::sync::Arc;

use bmo_chat_client::{ChatCapability, ChatModeStanding, ChatSend, ChatSendRequest, ChatVoice};
use bmo_receiver::roads::Facts;
use serde_json::{Value, json};

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, post, stood_on};
use crate::integration::common::units::{OWNER, StandInChat, registration};

// A service that lists its account's rooms and answers as the person it reads, and one that does neither.
async fn front(scratch: &Scratch) -> (Front, Arc<StandInChat>, Arc<StandInChat>) {
  let listing = Arc::new(
    StandInChat::new("road-a")
      .holding(&[-300, -200, -100])
      .titling(-200, "Second Room")
      .contacting(51, "Contact One", Some(-100))
      .contacting(52, "Contact Two", None)
      .contacting(53, "Contact Three", None)
      .canning(&[
        ChatCapability::RoomList,
        ChatCapability::ContactList,
        ChatCapability::StartChat,
        ChatCapability::StartGroup,
      ]),
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
  let can = |at: usize| -> BTreeSet<String> {
    serde_json::from_value(every["items"][at]["can"].clone()).expect("a provider's verbs")
  };
  let verbs =
    |names: &[&str]| -> BTreeSet<String> { names.iter().map(|name| (*name).to_owned()).collect() };

  assert_eq!(status.as_u16(), 200, "{every}");
  // Each provider's verbs are its own declaration's, whatever room it is on, so two of one room stand apart.
  assert_eq!(can(0), Facts::record().can.into_iter().collect::<BTreeSet<String>>());
  assert_eq!(
    can(1),
    verbs(&["text", "delete", "pin", "roomList", "contactList", "startChat", "startGroup"])
  );
  assert_eq!(can(2), verbs(&["text", "delete", "pin"]));
  assert_eq!(
    every["items"],
    json!([
      { "key": "record", "label": every["items"][0]["label"], "can": every["items"][0]["can"],
        "listsRooms": false, "listsContacts": false,
        "startsChats": false, "startsGroups": false, "roomModes": [], "fileBytes": null },
      { "key": "road-a", "label": "the road-a road", "can": every["items"][1]["can"],
        "listsRooms": true, "listsContacts": true,
        "startsChats": true, "startsGroups": true, "roomModes": ["avatar", "record-only"], "fileBytes": null },
      { "key": "road-b", "label": "the road-b road", "can": every["items"][2]["can"],
        "listsRooms": false, "listsContacts": false,
        "startsChats": false, "startsGroups": false, "roomModes": ["record-only"], "fileBytes": null },
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
      "items": [{ "id": -200, "title": "Second Room", "kind": "group", "privateTo": null, "held": false,
        "mode": null }],
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
async fn a_service_that_cannot_be_reached_is_answered_upstream_failed_on_its_listings() {
  let scratch = Scratch::new().await;
  let down = Arc::new(StandInChat::new("road-down").down());
  let front = stood_on(&scratch, OWNER, vec![down.clone()]).await;

  for path in ["/providers/road-down/rooms", "/providers/road-down/contacts"] {
    let (answered, body) = get(&front, path).await;

    assert_eq!(
      (answered.as_u16(), body["code"].as_str()),
      (502, Some("upstream-failed")),
      "{path}: {body}"
    );
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
    json!({ "providers": ["road-a", "record"], "chosen": true, "fileLimit": null })
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

  // Room -100 is led by the account that owns the supervisor it names; room -200 names none.
  let mut leader = registration("agent-b", "Agent B", vec!["agent-b"], None);

  leader.row.insert("ownerAccount".to_owned(), json!(77));
  front.standing.agency.registering(vec![leader]);
  scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES (-100, 'agent-b')"#,
      vec![],
    )
    .await;

  // A line the record keeps goes on to the service, and a send on the service's road goes out the same way.
  receiver.speech().kept(-100, "kept in the record", None, None, None).await.unwrap();
  receiver
    .speech()
    .act("road-a", &ChatSendRequest::new(-100, ChatSend::Typing { on: true }))
    .await
    .unwrap();

  // The owner's own words keep the owner's voice, and a room for the user alone keeps the service's.
  let mut owners = ChatSendRequest::new(-100, ChatSend::Typing { on: true });

  owners.mode = Some(ChatVoice::HumanReader);
  receiver.speech().act("road-a", &owners).await.unwrap();
  receiver.speech().kept(-200, "for the user alone", None, None, None).await.unwrap();

  // Each send into the led room goes out in its leader's account, and one into a room led by no one names none.
  let voices: Vec<(i64, Option<ChatVoice>, Option<i64>)> = listing
    .sent()
    .iter()
    .map(|request| (request.room_id, request.mode, request.account_id))
    .collect();

  assert_eq!(
    voices,
    [
      (-100, Some(ChatVoice::HumanAvatar), Some(77)),
      (-100, Some(ChatVoice::HumanAvatar), Some(77)),
      (-100, Some(ChatVoice::HumanReader), Some(77)),
      (-200, None, None),
    ]
  );
}

#[tokio::test]
async fn a_room_the_receiver_holds_is_marked_held_whether_or_not_it_was_added() {
  let scratch = Scratch::new().await;
  let (front, _listing, _plain) = front(&scratch).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-300, 'Kept before')"#, vec![])
    .await;
  post(&front, "/rooms", adding("road-a", -100, "record-only")).await;

  let (status, page) = get(&front, "/providers/road-a/rooms").await;
  let marks: Vec<(i64, bool, Value)> = page["items"]
    .as_array()
    .unwrap()
    .iter()
    .map(|room| (room["id"].as_i64().unwrap(), room["held"].as_bool().unwrap(), room["mode"].clone()))
    .collect();

  assert_eq!(status.as_u16(), 200, "{page}");
  assert_eq!(
    marks,
    [
      (-300, true, Value::Null),
      (-200, false, Value::Null),
      (-100, true, json!("record-only"))
    ]
  );
}

#[tokio::test]
async fn the_contacts_of_an_account_are_relayed_with_their_room_held_or_not() {
  let scratch = Scratch::new().await;
  let (front, listing, _plain) = front(&scratch).await;

  post(&front, "/rooms", adding("road-a", -100, "record-only")).await;

  let (status, page) = get(&front, "/providers/road-a/contacts?limit=2").await;

  assert_eq!(status.as_u16(), 200, "{page}");
  assert_eq!(
    page,
    json!({
      "items": [
        { "id": 51, "name": "Contact One", "kind": "person", "roomId": -100, "held": true, "mode": "record-only" },
        { "id": 52, "name": "Contact Two", "kind": "person", "roomId": null, "held": false, "mode": null },
      ],
      "more": true,
    })
  );

  for (path, status, code) in [
    ("/providers/road-b/contacts", 409, "contacts-unlisted"),
    ("/providers/record/contacts", 409, "contacts-unlisted"),
    ("/providers/road-z/contacts", 404, "not-found"),
    ("/providers/road-a/contacts?limit=many", 400, "bad-request"),
    ("/providers/road-a/contacts?accountId=seven", 400, "bad-request"),
  ] {
    let (answered, body) = get(&front, path).await;

    assert_eq!((answered.as_u16(), body["code"].as_str()), (status, Some(code)), "{path}: {body}");
  }

  // The account a listing is for is handed to the service as it came, and a listing naming none names none.
  let (status, page) = get(&front, "/providers/road-a/contacts?accountId=77").await;

  assert_eq!((status.as_u16(), page["items"].as_array().map(Vec::len)), (200, Some(3)), "{page}");

  let accounts: Vec<Option<i64>> =
    listing.contacts_asked.lock().unwrap().iter().map(|query| query.account_id).collect();

  assert_eq!(accounts, [None, Some(77)]);
}

#[tokio::test]
async fn a_conversation_started_on_the_service_is_added_as_the_room_it_comes_to() {
  let scratch = Scratch::new().await;
  let (front, listing, _plain) = front(&scratch).await;

  let (status, room) = post(
    &front,
    "/rooms",
    json!({ "provider": "road-a", "contactId": 52, "mode": "record-only" }),
  )
  .await;

  assert_eq!(status.as_u16(), 201, "{room}");
  assert_eq!(
    (room["id"].as_i64(), room["title"].as_str(), room["mode"].as_str()),
    (Some(52), Some("Contact Two"), Some("record-only"))
  );
  assert_eq!(
    get(&front, "/rooms/52/providers").await.1,
    json!({ "providers": ["road-a", "record"], "chosen": true, "fileLimit": null })
  );

  let group = json!({
    "provider": "road-a", "contactIds": [51, 53], "title": "Plans", "mode": "avatar", "accountId": 77,
  });
  let (status, room) = post(&front, "/rooms", group).await;

  assert_eq!(status.as_u16(), 201, "{room}");
  assert_eq!(
    (room["id"].as_i64(), room["title"].as_str(), room["mode"].as_str()),
    (Some(-700), Some("Plans"), Some("avatar"))
  );

  // The account a start names is the one the service is asked to start it in; a start naming none names none.
  let accounts: Vec<Option<i64>> = listing.started().iter().map(|start| start.account_id).collect();

  assert_eq!(accounts, [None, Some(77)]);
}

#[tokio::test]
async fn a_start_the_service_does_not_declare_know_or_make_is_refused_and_writes_nothing() {
  let scratch = Scratch::new().await;
  let (front, listing, _plain) = front(&scratch).await;
  let start = |body: Value| {
    let mut body = body;

    body["provider"] = json!(body["provider"].as_str().unwrap_or("road-a"));
    body
  };

  for (body, status, code) in [
    (
      start(json!({ "provider": "road-b", "contactId": 52, "mode": "record-only" })),
      409,
      "chats-unstarted",
    ),
    (start(json!({ "contactId": 99, "mode": "record-only" })), 409, "contact-unknown"),
    (
      start(json!({ "contactIds": [52], "title": "Refused", "mode": "record-only" })),
      409,
      "start-refused",
    ),
    (start(json!({ "contactId": 52, "mode": "avatar" })), 409, "avatar-unsupported"),
    (start(json!({ "contactId": 51, "mode": "record-only" })), 201, ""),
    (start(json!({ "contactId": 51, "mode": "record-only" })), 409, "room-added"),
    (start(json!({ "contactIds": [52], "mode": "record-only" })), 400, "bad-request"),
    (
      start(json!({ "contactIds": [52], "title": " ", "mode": "record-only" })),
      400,
      "bad-request",
    ),
    (
      start(json!({ "contactId": 52, "roomId": -200, "mode": "record-only" })),
      400,
      "bad-request",
    ),
    (
      start(json!({ "roomId": -200, "accountId": 77, "mode": "record-only" })),
      400,
      "bad-request",
    ),
    (
      start(json!({ "contactId": 52, "accountId": "seven", "mode": "record-only" })),
      400,
      "bad-request",
    ),
  ] {
    let (answered, refused) = post(&front, "/rooms", body.clone()).await;

    assert_eq!(answered.as_u16(), status, "{body}: {refused}");

    if status != 201 {
      assert_eq!(refused["code"].as_str(), Some(code), "{body}: {refused}");
    }
  }

  // A read-only one-to-one takes no avatar mode, and nothing was started for a mode the service does not offer.
  listing.moding(ChatVoice::HumanAvatar, ChatModeStanding::Inactive);

  let (answered, refused) =
    post(&front, "/rooms", json!({ "provider": "road-a", "contactId": 53, "mode": "avatar" })).await;

  assert_eq!((answered.as_u16(), refused["code"].as_str()), (409, Some("avatar-unsupported")));

  let asked: Vec<String> = listing.started().iter().map(|start| format!("{:?}", start.start)).collect();

  assert_eq!(
    asked,
    [
      "OneToOne { contact_id: 99 }",
      "Group { title: \"Refused\", contact_ids: [52] }",
      "OneToOne { contact_id: 52 }",
      "OneToOne { contact_id: 51 }",
      "OneToOne { contact_id: 51 }",
    ]
  );

  let kept = scratch
    .texts(r#"SELECT "chat_id"::text || ' ' || "mode" FROM "room_modes" ORDER BY "chat_id""#)
    .await;

  assert_eq!(kept, ["-100 record-only"]);
}

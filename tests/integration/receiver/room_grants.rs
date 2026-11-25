//! A room member's rights over its people (docs/receiver.md, 「Console rooms and accounts」; docs/api.md, 「Rooms」):
//! a manager who governs the room grants and takes back the let-in and take-out rights, read by the console's members
//! list and the web unit's entry check. The standing that writes them is the one that locks a room, and the room is
//! none's private room, so an outsider's write is refused `not-room-leader`.

use std::sync::Arc;

use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, post, put, remove, stood_on};
use crate::integration::common::units::{AGENT, OWNER, StandInChat, registration};

const ROOM: i64 = -300;
const ABSENT: i64 = -999_999;
const ADMIN: i64 = 900;
const MEMBER: i64 = 55;
const OTHER: i64 = 555;
const LEADER: i64 = 81;
const OUTSIDER: i64 = 82;
const HEAD: i64 = 83;

// A front holding the room on one chat service, the web unit answering account `ADMIN` as the administrator.
async fn front(scratch: &Scratch) -> Front {
  let service = Arc::new(StandInChat::new("road-a").holding(&[ROOM]));
  let front = stood_on(scratch, OWNER, vec![service]).await;

  front.standing.web.admin(ADMIN.to_string());

  let (status, room) =
    post(&front, "/rooms", json!({ "provider": "road-a", "roomId": ROOM, "mode": "record-only" })).await;

  assert_eq!(status.as_u16(), 201, "{room}");
  scratch
    .run(
      &format!(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ({ROOM}, 'A room')"#),
      vec![],
    )
    .await;

  front
}

fn grants_path(account: i64, by: i64) -> String {
  format!("/rooms/{ROOM}/accounts/{account}/grants?by={by}")
}

// The rights as the read answers them, sans the asking account.
async fn rights(front: &Front, account: i64) -> serde_json::Value {
  let (status, body) = get(front, &grants_path(account, ADMIN)).await;

  assert_eq!(status.as_u16(), 200, "{body}");
  json!({ "letIn": body["letIn"], "takeOut": body["takeOut"] })
}

#[tokio::test]
async fn the_administrator_grants_a_right_and_it_stands_until_taken_back() {
  let scratch = Scratch::new().await;
  let front = front(&scratch).await;

  // None is granted to begin with, and the administrator may write.
  let (status, none) = get(&front, &grants_path(MEMBER, ADMIN)).await;

  assert_eq!(status.as_u16(), 200, "{none}");
  assert_eq!(none, json!({ "account": MEMBER, "letIn": false, "takeOut": false, "mayWrite": true }));

  // Granting the let-in right answers the rights as they now stand.
  let (status, granted) = put(
    &front,
    &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants"),
    json!({ "by": ADMIN, "right": "let-in" }),
  )
  .await;

  assert_eq!(status.as_u16(), 200, "{granted}");
  assert_eq!(granted, json!({ "account": MEMBER, "letIn": true, "takeOut": false }));
  assert_eq!(rights(&front, MEMBER).await, json!({ "letIn": true, "takeOut": false }));

  // Granting it again changes nothing.
  let (status, again) = put(
    &front,
    &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants"),
    json!({ "by": ADMIN, "right": "let-in" }),
  )
  .await;

  assert_eq!((status.as_u16(), again["letIn"].clone()), (200, json!(true)), "{again}");

  // The take-out right stands beside it.
  let (status, both) = put(
    &front,
    &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants"),
    json!({ "by": ADMIN, "right": "take-out" }),
  )
  .await;

  assert_eq!((status.as_u16(), both["takeOut"].clone()), (200, json!(true)), "{both}");
  assert_eq!(rights(&front, MEMBER).await, json!({ "letIn": true, "takeOut": true }));

  // One right taken back leaves the other.
  let (status, _) =
    remove(&front, &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants?by={ADMIN}&right=let-in")).await;

  assert_eq!(status.as_u16(), 204);
  assert_eq!(rights(&front, MEMBER).await, json!({ "letIn": false, "takeOut": true }));

  // The last right taken back takes the row with it; the read answers none again.
  let (status, _) =
    remove(&front, &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants?by={ADMIN}&right=take-out")).await;

  assert_eq!(status.as_u16(), 204);
  assert_eq!(rights(&front, MEMBER).await, json!({ "letIn": false, "takeOut": false }));
  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "account" FROM "room_grants" WHERE "chat_id" = {ROOM} AND "account" = {MEMBER}"#
      ))
      .await,
    Vec::<String>::new()
  );
}

#[tokio::test]
async fn only_one_who_governs_the_room_writes_the_rights() {
  let scratch = Scratch::new().await;
  let front = front(&scratch).await;

  // The owner of a supervisor the room names leads it; a head of that leader's unit leads it too; an account that
  // neither leads nor governs is refused, and the room's rights stand untouched.
  let mut own = registration("agent-c", "Agent C", vec!["agent-c"], None);

  own.row.insert("ownerAccount".to_owned(), json!(LEADER));
  front
    .standing
    .agency
    .registering(vec![registration(AGENT, "Agent A", vec!["agent-a"], None), own]);
  put(&front, &format!("/rooms/{ROOM}/supervisors"), json!({ "supervisors": ["agent-c"] })).await;

  // An outsider may not write, and the read says it may not.
  let (status, refused) = put(
    &front,
    &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants"),
    json!({ "by": OTHER, "right": "let-in" }),
  )
  .await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (403, Some("not-room-leader")), "{refused}");
  assert_eq!(get(&front, &grants_path(MEMBER, OTHER)).await.1["mayWrite"], json!(false));

  // The leader writes.
  let (status, _) = put(
    &front,
    &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants"),
    json!({ "by": LEADER, "right": "let-in" }),
  )
  .await;

  assert_eq!(status.as_u16(), 200);

  // A head of the leader's unit takes it back.
  front.standing.web.heads.lock().unwrap().insert((HEAD.to_string(), LEADER.to_string()));
  let (status, _) =
    remove(&front, &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants?by={HEAD}&right=let-in")).await;

  assert_eq!(status.as_u16(), 204);

  // An account that does not lead, though it sits in the leader's unit's company, is refused.
  let (status, refused) = put(
    &front,
    &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants"),
    json!({ "by": OUTSIDER, "right": "take-out" }),
  )
  .await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (403, Some("not-room-leader")), "{refused}");
}

#[tokio::test]
async fn a_refusal_names_the_bad_request_and_the_absent_room() {
  let scratch = Scratch::new().await;
  let front = front(&scratch).await;

  // No right, or another one, is refused and reaches nobody.
  for right in [json!(null), json!(""), json!("letIn"), json!("everything")] {
    let (status, refused) = put(
      &front,
      &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants"),
      json!({ "by": ADMIN, "right": right }),
    )
    .await;

    assert_eq!((status.as_u16(), refused["code"].as_str()), (400, Some("bad-request")), "{refused}");
  }

  // The write for a call naming no account is refused.
  let (status, refused) =
    put(&front, &format!("/rooms/{ROOM}/accounts/{MEMBER}/grants"), json!({ "right": "let-in" })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (400, Some("bad-request")), "{refused}");

  // A room the receiver does not hold has no rights to read or write.
  let (status, refused) =
    get(&front, &format!("/rooms/{ABSENT}/accounts/{MEMBER}/grants?by={ADMIN}")).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (404, Some("not-found")), "{refused}");

  let (status, refused) = put(
    &front,
    &format!("/rooms/{ABSENT}/accounts/{MEMBER}/grants"),
    json!({ "by": ADMIN, "right": "let-in" }),
  )
  .await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (404, Some("not-found")), "{refused}");
}

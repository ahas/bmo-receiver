//! A room's senders matched to its members (`docs/messages.md`, 「A sender matched to the room's members」): a
//! sender a road numbered before moves to the member of its nickname, so a message's face and the member sheet ask
//! for one number and find the picture the road keeps under it; a nickname two members share, the owner and an
//! agent move nowhere.

use std::sync::Arc;

use bmo_chat_client::ChatAvatarKind::Person;
use bmo_chat_client::{ChatCapability, ChatMember};
use bmo_receiver::all;
use bmo_receiver::receiver::avatars::walk;
use bmo_receiver::receiver::senders::{Matched, match_every, matched};
use serde_json::Value;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, stood_on};
use crate::integration::common::units::{OWNER, StandInChat};

use super::avatar_roads::{png, served};

const ROOM: i64 = -9_000_000_000_951;
const BEFORE: i64 = 9_000_000_000_952;
const MEMBER: i64 = 9_000_000_000_953;
const NEIGHBOUR: i64 = 9_000_000_000_954;
const TWIN_A: i64 = 9_000_000_000_955;
const TWIN_B: i64 = 9_000_000_000_956;
const AGENT: i64 = 9_000_000_000_957;
const ACCOUNT: i64 = 9_000_000_000_958;

const GREEN: [u8; 3] = [30, 200, 30];

fn member(person: i64, name: &str) -> ChatMember {
  ChatMember {
    person: Some(person),
    name: name.to_owned(),
    status_message: None,
  }
}

// A front on one road that holds the room and lists its members, the room titled so the receiver holds it.
async fn front(scratch: &Scratch, road: StandInChat, members: Vec<ChatMember>) -> Front {
  let road = road.holding(&[ROOM]).canning(&[ChatCapability::MemberList]).membering(ROOM, members);
  let front = stood_on(scratch, OWNER, vec![Arc::new(road)]).await;

  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'Matching Room')"#,
      vec![ROOM.into()],
    )
    .await;

  front
}

// A message kept in the room as one that came on the road, under a sender and the nickname it spoke under.
async fn came(scratch: &Scratch, message: i64, sender: i64, name: &str, agent: Option<&str>) {
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via",
                                 "agent_name")
         VALUES ($1, $2, $3, $4, 1000, 'said', 'road-a', $5)"#,
      vec![ROOM.into(), message.into(), sender.into(), name.into(), agent.map(str::to_owned).into()],
    )
    .await;
}

// The sender number the console reads off a message, the one its face is asked by.
async fn face_of(front: &Front, message: i64) -> i64 {
  let (status, answer) = get(front, &format!("/rooms/{ROOM}/messages/{message}")).await;

  assert_eq!(status.as_u16(), 200, "{answer}");
  answer["senderId"].as_i64().unwrap()
}

// The number the member sheet gives the member of a nickname.
async fn row_of(front: &Front, name: &str) -> i64 {
  let (status, answer) = get(front, &format!("/rooms/{ROOM}/members")).await;

  assert_eq!(status.as_u16(), 200, "{answer}");
  answer["members"]
    .as_array()
    .unwrap()
    .iter()
    .find(|row: &&Value| row["name"] == name)
    .and_then(|row| row["person"].as_i64())
    .unwrap()
}

// One column of the room's rows of a table, in order.
async fn column(scratch: &Scratch, sql: &str, name: &str) -> Vec<String> {
  all(scratch.db(), sql, vec![ROOM.into()])
    .await
    .unwrap()
    .iter()
    .map(|row| {
      row
        .try_get::<i64>("", name)
        .map(|n| n.to_string())
        .or_else(|_| row.try_get::<String>("", name))
    })
    .collect::<Result<_, _>>()
    .unwrap()
}

#[tokio::test]
async fn a_sender_a_road_numbered_before_wears_the_picture_its_member_number_keeps_on_both_surfaces() {
  let scratch = Scratch::new().await;
  let road = StandInChat::new("road-a").picturing(Person, MEMBER, 1_000, png(GREEN), true);
  let front = front(&scratch, road, vec![member(MEMBER, "Nick One"), member(NEIGHBOUR, "Nick Two")]).await;

  came(&scratch, 1, BEFORE, "Nick One", None).await;
  came(&scratch, 2, BEFORE, " Nick One ", None).await;
  came(&scratch, 3, NEIGHBOUR, "Nick Two", None).await;
  scratch
    .run(
      r#"INSERT INTO "muted_members" ("chat_id", "user_id") VALUES ($1, $2)"#,
      vec![ROOM.into(), BEFORE.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "message_reactions" ("chat_id", "message_id", "by", "emoji", "at") VALUES ($1, 3, $2, 'x', 1)"#,
      vec![ROOM.into(), BEFORE.to_string().into()],
    )
    .await;

  walk(&front.receiver(), "road-a").await;

  let (status, _) = get(&front, &format!("/people/{BEFORE}/avatar")).await;

  assert_eq!(status.as_u16(), 404, "the road keeps no picture under the number it gave before");
  assert_eq!(match_every(&front.receiver()).await, 1);

  for message in [1, 2] {
    let face = face_of(&front, message).await;

    assert_eq!(face, MEMBER, "message {message}");
    assert_eq!(row_of(&front, "Nick One").await, face, "the sheet and the message ask one number");
    served(&front, &format!("/people/{face}/avatar"), GREEN).await;
  }
  assert_eq!(face_of(&front, 3).await, NEIGHBOUR, "a sender the list already names stays");
  assert_eq!(
    column(
      &scratch,
      r#"SELECT "sender_name" FROM "messages" WHERE "chat_id" = $1 ORDER BY "message_id""#,
      "sender_name"
    )
    .await,
    vec!["Nick One", " Nick One ", "Nick Two"],
    "the names the messages were archived under stay"
  );
  assert_eq!(
    column(&scratch, r#"SELECT "user_id" FROM "muted_members" WHERE "chat_id" = $1"#, "user_id").await,
    vec![MEMBER.to_string()]
  );
  assert_eq!(
    column(&scratch, r#"SELECT "by" FROM "message_reactions" WHERE "chat_id" = $1"#, "by").await,
    vec![MEMBER.to_string()]
  );

  // Started again, nothing is left to match.
  assert_eq!(match_every(&front.receiver()).await, 0);
}

#[tokio::test]
async fn a_nickname_two_members_share_the_owner_and_an_agent_are_never_moved() {
  let scratch = Scratch::new().await;
  let front = front(
    &scratch,
    StandInChat::new("road-a"),
    vec![
      member(TWIN_A, "Twin"),
      member(TWIN_B, "Twin"),
      member(ACCOUNT, "Owner Nick"),
      member(MEMBER, "Agent Nick"),
    ],
  )
  .await;

  came(&scratch, 1, BEFORE, "Twin", None).await;
  came(&scratch, 2, OWNER, "Owner Nick", None).await;
  came(&scratch, 3, AGENT, "Agent Nick", Some("agent-a")).await;
  came(&scratch, 4, AGENT, "Agent Nick", None).await;
  came(&scratch, 5, NEIGHBOUR, "Gone", None).await;

  assert_eq!(
    matched(&front.receiver(), ROOM).await,
    Some(Matched {
      moved: vec![],
      ambiguous: vec!["Twin".to_owned()],
    })
  );
  assert_eq!(
    column(
      &scratch,
      r#"SELECT "sender_id" FROM "messages" WHERE "chat_id" = $1 ORDER BY "message_id""#,
      "sender_id"
    )
    .await,
    [BEFORE, OWNER, AGENT, AGENT, NEIGHBOUR].map(|n| n.to_string()).to_vec()
  );
}

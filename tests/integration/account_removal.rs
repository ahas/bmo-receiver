//! An account's removal and the administrator's removal of any room (docs/receiver.md, 「An account's removal」 and
//! 「Removing a room's registration」): the administrator removes another account's private room and a sealed one,
//! while anyone else meets the guard; an account's removal counts what it would take with nothing written, then
//! takes exactly that, each private room whole, its memberships of the rooms that stay and its record identity, and
//! leaves every other account's rows and the rows of no account standing; a second call takes nothing; its number is
//! never handed out again; a refused private room stops the call with what was taken before staying taken; every
//! caller but the administrator under the console's token is refused; and an organisation unit's removal takes
//! nothing.

use std::sync::{Arc, Mutex};

use axum::{
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use serde_json::{Value, json};
use tempfile::tempdir;
use tower::ServiceExt;

use bmo_receiver::accounts::identity;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::routes::routes;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, Kicked, RECEIVER_TOKEN, TOKEN, as_console};
use crate::integration::common::units::{StandInChat, deployment, standing_on};

// The administrator's account, as the web unit knows him.
const ADMIN: &str = "1";
// The account removed, a member of the shared room, and another account whose rows stand.
const LEAVING: i64 = 7;
const MEMBER: i64 = 8;
const OTHER: i64 = 9;
// The leaving account's private rooms, the shared room it is a member of, the other account's private room and a
// sealed room, which names no account.
const OWN_A: i64 = -4_410;
const OWN_B: i64 = -4_411;
const SHARED: i64 = -4_420;
const OTHERS: i64 = -4_430;
const SEALED: i64 = -4_440;
// The room the chat service says is the administrator's own private chat.
const OWNERS: i64 = -4_302;

/// The receiver's routes for the administrator, under the console's token, on a data root of their own.
async fn bench() -> (Scratch, Front) {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let chats = vec![Arc::new(StandInChat::new("road-a").holding(&[OWNERS]).owners(OWNERS))];
  let mut given = deployment();

  given.owner_room = Some(OWNERS);

  let standing = standing_on(scratch.db(), DataRoot::new(data.path()), chats, given);
  let place: Arc<str> = scratch.address().into();

  standing.receiver.hold_place(place);
  standing.web.admin(ADMIN.to_owned());

  let kicked = Arc::new(Kicked(Mutex::default()));

  standing.receiver.hold_pass(kicked.clone());

  (
    scratch,
    Front {
      routes: routes(standing.receiver.clone(), as_console(TOKEN)),
      data,
      standing,
      kicked,
    },
  )
}

/// A request asked of the front under a token, with the headers given.
async fn asked(
  front: &Front,
  method: &str,
  path: &str,
  token: &str,
  headers: &[(&str, &str)],
) -> (StatusCode, Value) {
  let request = headers.iter().fold(
    Request::builder()
      .method(method)
      .uri(path)
      .header("authorization", format!("Bearer {token}")),
    |request, (name, value)| request.header(*name, *value),
  );
  let answer = front.routes.clone().oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

/// The same, as the administrator under the console's token.
async fn as_administrator(front: &Front, method: &str, path: &str) -> (StatusCode, Value) {
  asked(front, method, path, TOKEN, &[("Owner-Key", ADMIN), ("Owner-Role", "administrator")]).await
}

/// A room with a title, a message and a folder in each of the three stores, kept private to the account given, or
/// sealed where none is.
async fn private_room(scratch: &Scratch, data: &std::path::Path, room: i64, account: Option<i64>) {
  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'a room')"#,
      vec![room.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "private_rooms" ("chat_id", "account", "written_at") VALUES ($1, $2, 1)"#,
      vec![room.into(), account.into()],
    )
    .await;
  scratch.message(room, 1, 70).await;

  for folder in ["collected", "images", "outbox"] {
    let at = data.join(folder).join(room.to_string());

    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(at.join("file.txt"), "a file").unwrap();
  }
}

async fn rows(scratch: &Scratch, sql: &str, values: Vec<sea_orm::Value>) -> i64 {
  scratch.count(sql, values).await
}

async fn titled(scratch: &Scratch, room: i64) -> i64 {
  rows(scratch, r#"SELECT count(*) FROM "chat_titles" WHERE "chat_id" = $1"#, vec![room.into()]).await
}

#[tokio::test]
async fn the_administrator_removes_another_accounts_private_room_and_a_sealed_one_and_nobody_else_does() {
  let (scratch, front) = bench().await;
  let data = front.data.path().to_path_buf();

  private_room(&scratch, &data, OWN_A, Some(LEAVING)).await;
  private_room(&scratch, &data, SEALED, None).await;

  let (status, refused) = asked(&front, "DELETE", &format!("/rooms/{OWN_A}?by={MEMBER}"), TOKEN, &[]).await;

  assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
  assert_eq!(refused["code"], "private-room", "another account meets the guard first");

  let (status, refused) =
    asked(&front, "DELETE", &format!("/rooms/{OWN_A}?by={LEAVING}"), TOKEN, &[]).await;

  assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
  assert_eq!(refused["code"], "not-administrator", "its own account is no administrator");
  assert_eq!(titled(&scratch, OWN_A).await, 1, "a refusal writes nothing");

  for room in [OWN_A, SEALED] {
    let (status, shown) =
      asked(&front, "DELETE", &format!("/rooms/{room}?by={ADMIN}&dryRun=true"), TOKEN, &[]).await;

    assert_eq!(status, StatusCode::OK, "{room}: {shown}");
    assert_eq!(titled(&scratch, room).await, 1, "the preview writes nothing");

    let (status, taken) = asked(&front, "DELETE", &format!("/rooms/{room}?by={ADMIN}"), TOKEN, &[]).await;

    assert_eq!(status, StatusCode::OK, "{room}: {taken}");
    assert_eq!(titled(&scratch, room).await, 0, "{room}");
    assert_eq!(
      rows(
        &scratch,
        r#"SELECT count(*) FROM "private_rooms" WHERE "chat_id" = $1"#,
        vec![room.into()]
      )
      .await,
      0,
      "{room}"
    );
    assert!(!data.join("collected").join(room.to_string()).exists(), "{room}");
  }
}

#[tokio::test]
async fn an_accounts_removal_is_counted_then_taken_as_counted_and_nothing_of_anyone_else_moves() {
  let (scratch, front) = bench().await;
  let data = front.data.path().to_path_buf();
  let db = scratch.db();

  // The other account's identity first, so the leaving one's is the lowest number the record gave.
  let others = identity(db, OTHER).await.unwrap();
  let leaving = identity(db, LEAVING).await.unwrap();
  let picture = data.join("avatars").join("set").join("person").join(format!("{leaving}.jpg"));

  assert!(leaving < others);

  for (room, account) in
    [(OWN_A, Some(LEAVING)), (OWN_B, Some(LEAVING)), (OTHERS, Some(OTHER)), (SEALED, None)]
  {
    private_room(&scratch, &data, room, account).await;
  }

  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'shared')"#,
      vec![SHARED.into()],
    )
    .await;
  scratch.message(SHARED, 1, leaving).await;

  for (sql, values) in [
    (
      r#"INSERT INTO "room_accounts" ("chat_id", "account", "added_by", "added_at") VALUES ($1, $2, $3, 1), ($1, $3, $3, 1)"#,
      vec![SHARED.into(), LEAVING.into(), MEMBER.into()],
    ),
    (
      r#"INSERT INTO "room_accounts" ("chat_id", "account", "added_by", "added_at") VALUES ($1, $2, $2, 1)"#,
      vec![OWN_A.into(), LEAVING.into()],
    ),
    (
      r#"INSERT INTO "muted_members" ("chat_id", "user_id") VALUES ($1, $2), ($3, $2)"#,
      vec![SHARED.into(), leaving.into(), OWN_B.into()],
    ),
    (
      r#"INSERT INTO "room_grants" ("chat_id", "account", "let_in", "take_out") VALUES ($1, $2, TRUE, FALSE)"#,
      vec![SHARED.into(), LEAVING.into()],
    ),
    (
      r#"INSERT INTO "people_names" ("sender_id", "name", "named_at") VALUES ($1, 'seven', 1), ($2, 'nine', 1)"#,
      vec![leaving.into(), others.into()],
    ),
    (
      r#"INSERT INTO "avatar_sets" ("kind", "subject_id", "path", "set_at") VALUES ('person', $1, $2, 1)"#,
      vec![leaving.into(), picture.display().to_string().into()],
    ),
  ] {
    scratch.run(sql, values).await;
  }

  std::fs::create_dir_all(picture.parent().unwrap()).unwrap();
  std::fs::write(&picture, "jpeg").unwrap();

  let (status, counted) = as_administrator(&front, "GET", &format!("/accounts/{LEAVING}/removal")).await;
  let expected = json!({
    "private-rooms": 2,
    "memberships": 1,
    "grants": 1,
    "identities": 1,
    "names": 1,
    "mutes": 1,
    "pictures": 1,
  });

  assert_eq!(status, StatusCode::OK, "{counted}");
  assert_eq!(counted, json!({ "counts": expected }));
  assert_eq!(titled(&scratch, OWN_A).await, 1, "the count writes nothing");
  assert!(picture.exists());

  let (status, removed) = as_administrator(&front, "DELETE", &format!("/accounts/{LEAVING}")).await;

  assert_eq!(status, StatusCode::OK, "{removed}");
  assert_eq!(removed, json!({ "removed": expected }), "what was counted is what was taken");

  for room in [OWN_A, OWN_B] {
    assert_eq!(titled(&scratch, room).await, 0, "{room}");
    assert!(!data.join("collected").join(room.to_string()).exists(), "{room}");
  }

  assert!(!picture.exists());

  for (sql, values, standing) in [
    (r#"SELECT count(*) FROM "private_rooms" WHERE "account" = $1"#, vec![LEAVING.into()], 0),
    (r#"SELECT count(*) FROM "room_accounts" WHERE "account" = $1"#, vec![LEAVING.into()], 0),
    (r#"SELECT count(*) FROM "room_accounts" WHERE "account" = $1"#, vec![MEMBER.into()], 1),
    (r#"SELECT count(*) FROM "room_grants" WHERE "account" = $1"#, vec![LEAVING.into()], 0),
    (
      r#"SELECT count(*) FROM "provider_users" WHERE "sender_id" = $1"#,
      vec![leaving.into()],
      0,
    ),
    (r#"SELECT count(*) FROM "people_names" WHERE "sender_id" = $1"#, vec![leaving.into()], 0),
    (r#"SELECT count(*) FROM "muted_members" WHERE "user_id" = $1"#, vec![leaving.into()], 0),
    (r#"SELECT count(*) FROM "avatar_sets" WHERE "subject_id" = $1"#, vec![leaving.into()], 0),
    (
      r#"SELECT count(*) FROM "messages" WHERE "chat_id" = $1 AND "sender_id" = $2"#,
      vec![SHARED.into(), leaving.into()],
      1,
    ),
    (r#"SELECT count(*) FROM "chat_titles" WHERE "chat_id" = $1"#, vec![SHARED.into()], 1),
    (
      r#"SELECT count(*) FROM "provider_users" WHERE "sender_id" = $1"#,
      vec![others.into()],
      1,
    ),
    (r#"SELECT count(*) FROM "people_names" WHERE "sender_id" = $1"#, vec![others.into()], 1),
    (r#"SELECT count(*) FROM "private_rooms" WHERE "chat_id" = $1"#, vec![OTHERS.into()], 1),
    (
      r#"SELECT count(*) FROM "private_rooms" WHERE "chat_id" = $1 AND "account" IS NULL"#,
      vec![SEALED.into()],
      1,
    ),
  ] {
    assert_eq!(rows(&scratch, sql, values.clone()).await, standing, "{sql} {values:?}");
  }

  assert!(data.join("collected").join(OTHERS.to_string()).exists());
  assert!(data.join("collected").join(SEALED.to_string()).exists());

  let zeros = json!({
    "private-rooms": 0,
    "memberships": 0,
    "grants": 0,
    "identities": 0,
    "names": 0,
    "mutes": 0,
    "pictures": 0,
  });

  assert_eq!(
    as_administrator(&front, "DELETE", &format!("/accounts/{LEAVING}")).await,
    (StatusCode::OK, json!({ "removed": zeros })),
    "a second call takes nothing"
  );
  assert_eq!(
    as_administrator(&front, "GET", &format!("/accounts/{LEAVING}/removal")).await,
    (StatusCode::OK, json!({ "counts": zeros }))
  );

  let next = identity(db, 77).await.unwrap();

  assert!(
    next < leaving,
    "the removed number {leaving} is never handed out again, and {next} is below it"
  );
}

#[tokio::test]
async fn a_refused_private_room_stops_the_removal_and_what_was_taken_before_stays_taken() {
  let (scratch, front) = bench().await;
  let data = front.data.path().to_path_buf();
  let below = OWNERS - 100;

  private_room(&scratch, &data, below, Some(LEAVING)).await;
  private_room(&scratch, &data, OWNERS, Some(LEAVING)).await;
  scratch
    .run(
      r#"INSERT INTO "room_accounts" ("chat_id", "account", "added_by", "added_at") VALUES ($1, $2, $2, 1)"#,
      vec![SHARED.into(), LEAVING.into()],
    )
    .await;

  let (status, refused) = as_administrator(&front, "DELETE", &format!("/accounts/{LEAVING}")).await;

  assert_eq!(status, StatusCode::CONFLICT, "{refused}");
  assert_eq!(refused["code"], "owners-chat");
  assert!(refused["error"].as_str().unwrap().contains(&OWNERS.to_string()), "{refused}");
  assert_eq!(titled(&scratch, below).await, 0, "the room before it was taken");
  assert_eq!(titled(&scratch, OWNERS).await, 1, "the refused room stands");
  assert_eq!(
    rows(
      &scratch,
      r#"SELECT count(*) FROM "room_accounts" WHERE "account" = $1"#,
      vec![LEAVING.into()]
    )
    .await,
    1,
    "nothing after the refused room was taken"
  );
}

#[tokio::test]
async fn anyone_but_the_administrator_under_the_consoles_token_is_refused_and_nothing_is_written() {
  let (scratch, front) = bench().await;
  let data = front.data.path().to_path_buf();
  let path = format!("/accounts/{LEAVING}");

  private_room(&scratch, &data, OWN_A, Some(LEAVING)).await;

  let administrator = [("Owner-Key", ADMIN), ("Owner-Role", "administrator")];
  let member = [("Owner-Key", "8"), ("Owner-Role", "administrator")];
  let roleless = [("Owner-Key", ADMIN)];

  for (token, headers) in [
    (RECEIVER_TOKEN, &administrator[..]),
    (TOKEN, &member[..]),
    (TOKEN, &roleless[..]),
    (TOKEN, &[][..]),
  ] {
    for method in ["GET", "DELETE"] {
      let at = if method == "GET" { format!("{path}/removal") } else { path.clone() };
      let (status, refused) = asked(&front, method, &at, token, headers).await;

      assert_eq!(status, StatusCode::FORBIDDEN, "{method} {token} {headers:?}: {refused}");
      assert_eq!(refused["code"], "forbidden");
    }
  }

  for bad in ["seven", "0", "-7", "+7", "7.0"] {
    assert_eq!(
      as_administrator(&front, "DELETE", &format!("/accounts/{bad}")).await.0,
      StatusCode::BAD_REQUEST,
      "{bad}"
    );
  }

  assert_eq!(titled(&scratch, OWN_A).await, 1, "a refusal writes nothing");
}

#[tokio::test]
async fn an_organisation_units_removal_takes_nothing() {
  let (_scratch, front) = bench().await;

  assert_eq!(
    as_administrator(&front, "GET", "/org-units/3/removal?company=2").await,
    (StatusCode::OK, json!({ "counts": {} }))
  );
  assert_eq!(
    as_administrator(&front, "DELETE", "/org-units/3?company=2").await,
    (StatusCode::OK, json!({ "removed": {} }))
  );
  assert_eq!(
    as_administrator(&front, "DELETE", "/org-units/3?company=two").await.0,
    StatusCode::BAD_REQUEST
  );
  assert_eq!(as_administrator(&front, "GET", "/org-units/0/removal").await.0, StatusCode::BAD_REQUEST);
  assert_eq!(
    asked(
      &front,
      "DELETE",
      "/org-units/3?company=2",
      RECEIVER_TOKEN,
      &[("Owner-Key", ADMIN), ("Owner-Role", "administrator")]
    )
    .await
    .0,
    StatusCode::FORBIDDEN
  );
}

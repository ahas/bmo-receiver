//! A profile picture set from the console (docs/pictures.md, 「Profile pictures」): set on a room, a person or an
//! agent under every number it stands under, kept as a JPEG beside the picture a chat service hands over, the newer
//! of the two served and listed, let go to the handed-over one, and refused for an unknown kind, no picture or no
//! agent's number.

use axum::{
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use serde_json::Value;
use tower::ServiceExt;

use crate::common::Scratch;
use crate::common::queue::{Front, TOKEN, stood};
use crate::common::units::OWNER;

// A small picture with transparency, as a PNG.
fn png() -> Vec<u8> {
  let picture = image::RgbaImage::from_pixel(4, 4, image::Rgba([10, 20, 30, 128]));
  let mut bytes = std::io::Cursor::new(Vec::new());

  picture.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
  bytes.into_inner()
}

// A form with the bytes under the field named.
fn form(field: &str, bytes: &[u8]) -> Vec<u8> {
  let mut body = format!(
    "--own\r\ncontent-type: image/png\r\ncontent-disposition: form-data; name=\"{field}\"; filename=\"p.png\"\r\n\r\n"
  )
  .into_bytes();

  body.extend_from_slice(bytes);
  body.extend_from_slice(b"\r\n--own--\r\n");
  body
}

async fn put(front: &Front, path: &str, body: Vec<u8>) -> (StatusCode, Value) {
  put_as(front, path, body, ADMINISTRATOR).await
}

// The console relay's caller: the signed-in login's user id and its role.
const ADMINISTRATOR: (&str, &str) = ("user-1", "administrator");
const MEMBER: (&str, &str) = ("user-2", "member");

async fn put_as(front: &Front, path: &str, body: Vec<u8>, caller: (&str, &str)) -> (StatusCode, Value) {
  let asked = Request::put(path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("owner-key", caller.0)
    .header("owner-role", caller.1)
    .header("content-type", "multipart/form-data; boundary=own")
    .body(Body::from(body))
    .unwrap();
  let answer = front.routes.clone().oneshot(asked).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

async fn bytes_of(front: &Front, path: &str) -> (StatusCode, Vec<u8>) {
  let asked = Request::get(path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .body(Body::empty())
    .unwrap();
  let answer = front.routes.clone().oneshot(asked).await.unwrap();
  let status = answer.status();

  (status, to_bytes(answer.into_body(), usize::MAX).await.unwrap().to_vec())
}

async fn delete(front: &Front, path: &str) -> (StatusCode, Value) {
  let asked = Request::delete(path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("owner-key", ADMINISTRATOR.0)
    .header("owner-role", ADMINISTRATOR.1)
    .body(Body::empty())
    .unwrap();
  let answer = front.routes.clone().oneshot(asked).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

#[tokio::test]
async fn a_picture_set_on_a_room_a_person_or_an_agent_stands_beside_the_handed_over_one_the_newer_standing()
{
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  // A person and a room: each kept as a JPEG, served by the route every room reads.
  for (path, served) in [
    ("/avatars/person/55", "/people/55/avatar"),
    ("/avatars/room/-100", "/rooms/-100/avatar"),
  ] {
    let (status, answer) = put(&front, path, form("picture", &png())).await;

    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["items"].as_array().unwrap().len(), 1, "{answer}");

    let (status, bytes) = bytes_of(&front, served).await;

    assert_eq!(status, StatusCode::OK, "{served}");
    assert_eq!(&bytes[..3], &[0xff, 0xd8, 0xff], "the picture is kept as a JPEG");
  }

  // A chat service hands over a newer picture of the person: it stands, and the list names the person once.
  let handed = front.data.path().join("avatars").join("person");

  std::fs::create_dir_all(&handed).unwrap();
  std::fs::write(handed.join("55.jpg"), b"handed over").unwrap();
  scratch
    .run(
      &format!(
        r#"INSERT INTO "avatars" ("kind", "subject_id", "photo_id", "path", "updated_at") VALUES ('person', 55, 'p', '{}', 9999999999999)"#,
        handed.join("55.jpg").display()
      ),
      vec![],
    )
    .await;
  assert_eq!(
    bytes_of(&front, "/people/55/avatar").await.1,
    b"handed over".to_vec(),
    "the newer one stands"
  );

  let (_, listed) = bytes_of(&front, "/avatars").await;
  let listed: Value = serde_json::from_slice(&listed).unwrap();
  let people: Vec<&Value> = listed["items"]
    .as_array()
    .unwrap()
    .iter()
    .filter(|item| item["kind"] == "person" && item["id"] == 55)
    .collect();

  assert_eq!(people.len(), 1, "each subject is listed once: {listed}");
  assert_eq!(people[0]["takenAt"], 9999999999999_i64);

  // The room's set picture let go: nothing of its own was handed over, so none stands.
  let (status, gone) = delete(&front, "/avatars/room/-100").await;

  assert_eq!(status, StatusCode::OK, "{gone}");
  assert_eq!(bytes_of(&front, "/rooms/-100/avatar").await.0, StatusCode::NOT_FOUND);

  // An agent: under every number it stands under, a road's and the one its words are archived under.
  scratch
    .run(r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('record', 'agent-x', -7)"#, vec![])
    .await;
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via", "agent_name")
         VALUES (-100, 1, 77, 'Agent X', 1, 'hello', 'road-a', 'agent-x')"#,
      vec![],
    )
    .await;

  let (status, answer) = put(&front, "/avatars/agent/agent-x", form("picture", &png())).await;
  let ids: Vec<i64> = answer["items"]
    .as_array()
    .unwrap()
    .iter()
    .filter_map(|item| item["id"].as_i64())
    .collect();

  assert_eq!(status, StatusCode::OK, "{answer}");
  assert_eq!(ids, vec![-7, 77]);
  assert_eq!(bytes_of(&front, "/people/77/avatar").await.0, StatusCode::OK);

  // Refusals: an unknown kind, no picture, not a picture, an agent under no number.
  assert_eq!(put(&front, "/avatars/thing/1", form("picture", &png())).await.1["code"], "unknown-kind");
  assert_eq!(
    put(&front, "/avatars/person/55", form("other", &png())).await.1["code"],
    "no-picture-given"
  );
  assert_eq!(
    put(&front, "/avatars/person/55", form("picture", b"not a picture")).await.1["code"],
    "not-a-picture"
  );
  assert_eq!(
    put(&front, "/avatars/agent/agent-none", form("picture", &png())).await.1["code"],
    "no-number"
  );
}

#[tokio::test]
async fn a_member_sets_his_own_picture_alone_and_the_administrator_every_one() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  // The member's own sender, as the account model links it to his user.
  front.standing.web.linking(55, MEMBER.0.to_owned());

  assert_eq!(
    put_as(&front, "/avatars/person/55", form("picture", &png()), MEMBER).await.0,
    StatusCode::OK
  );

  for path in ["/avatars/person/56", "/avatars/room/-100", "/avatars/agent/agent-x"] {
    let (status, answer) = put_as(&front, path, form("picture", &png()), MEMBER).await;

    assert_eq!(
      (status, answer["code"].as_str()),
      (StatusCode::FORBIDDEN, Some("picture-forbidden")),
      "{path}"
    );
  }

  let (status, answer) = put_as(&front, "/avatars/room/-100", form("picture", &png()), ("", "")).await;

  assert_eq!(
    (status, answer["code"].as_str()),
    (StatusCode::FORBIDDEN, Some("picture-forbidden")),
    "no caller named"
  );
  assert_eq!(
    put_as(&front, "/avatars/room/-100", form("picture", &png()), ADMINISTRATOR).await.0,
    StatusCode::OK
  );
}

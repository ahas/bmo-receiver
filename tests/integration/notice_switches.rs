//! The owner's private chat's notice switches (docs/api.md, 「Notices」; docs/voiced-notices.md, 「Operating notices
//! per chat」): both read as on until set off, moved one at a time by their kind, an unknown kind and a body whose
//! `on` is not a boolean refused.

use axum::http::StatusCode;
use bmo_receiver::receiver::wake::rest::NOTICE;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{get, put, stood};
use crate::integration::common::units::OWNER;

#[tokio::test]
async fn the_switches_read_as_on_and_move_one_at_a_time_by_their_kind() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  assert_eq!(
    get(&front, "/notice-switches").await,
    (StatusCode::OK, json!({ "restart": true, "rest": true }))
  );

  let (status, both) = put(&front, "/notice-switches/rest", json!({ "on": false })).await;

  assert_eq!((status, both), (StatusCode::OK, json!({ "restart": true, "rest": false })));

  let (_, both) = put(&front, "/notice-switches/restart", json!({ "on": false })).await;

  assert_eq!(both, json!({ "restart": false, "rest": false }));

  let (status, body) = put(&front, "/notice-switches/weather", json!({ "on": false })).await;

  assert_eq!((status, body["code"].as_str()), (StatusCode::NOT_FOUND, Some("not-found")));

  let (status, body) = put(&front, "/notice-switches/rest", json!({ "on": "off" })).await;

  assert_eq!((status, body["code"].as_str()), (StatusCode::BAD_REQUEST, Some("bad-request")));

  put(&front, "/notice-switches/rest", json!({ "on": true })).await;

  assert_eq!(get(&front, "/notice-switches").await.1, json!({ "restart": false, "rest": true }));
  assert_eq!(NOTICE, "rest");
}

//! The owner's private chat's notice switches (docs/api.md, 「Notices」; docs/voiced-notices.md, 「Operating notices
//! per chat」): `restart`, the going-down line and the back notice, and `rest`, the rest's line and its lifting's
//! notice, read and moved one at a time from the console.

use axum::{
  body::Bytes,
  extract::{Path, State},
  http::StatusCode,
};
use serde_json::{Value, json};

use crate::chat::restart::notice_enabled;
use crate::receiver::rows;
use crate::routes::{Answered, Served, answered, refused};

/// The kinds of notice a switch stands for.
pub const KINDS: [&str; 2] = ["restart", "rest"];

async fn standing(served: &Served) -> Answered {
  let receiver = &served.receiver;
  let owner = receiver.owner_room();

  answered(json!({
    "restart": notice_enabled(receiver.db(), owner, "restart").await,
    "rest": notice_enabled(receiver.db(), owner, "rest").await,
  }))
}

/// `GET /notice-switches`: both switches as they stand.
pub async fn switches(State(served): State<Served>) -> Answered {
  standing(&served).await
}

/// `PUT /notice-switches/{kind}`: one switch moved, answered with both as they now stand.
pub async fn switch(State(served): State<Served>, Path(kind): Path<String>, body: Bytes) -> Answered {
  if !KINDS.contains(&kind.as_str()) {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("No notice switch is named {kind}."),
    ));
  }

  let on = serde_json::from_slice::<Value>(&body)
    .ok()
    .and_then(|asked| asked.get("on").and_then(Value::as_bool))
    .ok_or_else(|| refused(StatusCode::BAD_REQUEST, "bad-request", "on is a boolean."))?;
  let receiver = &served.receiver;

  rows::set_notice(receiver.db(), receiver.owner_room(), &kind, on)
    .await
    .map_err(|why| refused(StatusCode::INTERNAL_SERVER_ERROR, "failed", &why.to_string()))?;

  standing(&served).await
}

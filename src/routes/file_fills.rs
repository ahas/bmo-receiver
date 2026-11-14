//! A pass over a room's placeholder rows, asking its chat service for their files again (docs/room-files.md, 「Files
//! filled in later」; docs/api.md, 「Messages」).

use axum::body::Bytes;
use axum::extract::{Path, State};
use receiver_client::FileFill;

use super::{Answered, Served, answered, code, read, store};
use crate::receiver::Receiver;
use crate::receiver::file_fills::{self, FillRefused};

/// `POST /rooms/{roomId}/file-fills`: the rows the pass reached, each with what the road answered and the files
/// kept; an empty body asks the policy's default.
pub async fn fill(State(served): State<Served>, Path(room): Path<i64>, body: Bytes) -> Answered {
  let asked = match body.is_empty() {
    true => FileFill::default(),
    false => read::<FileFill>(&body)?,
  };
  let receiver: Receiver = (*served.receiver).clone();

  match file_fills::fill(&receiver, room, &asked).await {
    Ok(filled) => answered(serde_json::to_value(filled).unwrap_or_default()),
    Err(FillRefused::Refused(why)) => Err(code(&why)),
    Err(FillRefused::Store(why)) => Err(store(why)),
  }
}

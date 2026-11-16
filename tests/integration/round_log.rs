//! A room's round log as a reader asks it (docs/drafting.md, 「The round log」): the rounds it names, or, where it
//! names none, the room's newest rounds.

use axum::http::StatusCode;
use serde_json::Value;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{get, stood};
use crate::integration::common::units::{AGENT, OWNER};

const ROOM: i64 = -900;

// Rows of the round log: each round's rows, written in the order given.
async fn written(scratch: &Scratch, rows: &[(i64, &str)]) {
  for (number, (room, round)) in rows.iter().enumerate() {
    scratch
      .run(
        r#"INSERT INTO "draft_rounds" ("round", "chat_id", "agent", "number", "question", "kind", "answer_chars",
                                       "changed", "outcome", "ms", "at")
           VALUES ($1, $2, $3, $4, 'draft', 'turn', 10, false, 'send', 5, 1000)"#,
        vec![(*round).into(), (*room).into(), AGENT.into(), (number as i64 + 1).into()],
      )
      .await;
  }
}

// The rounds a page holds, in its order.
fn rounds(page: &Value) -> Vec<String> {
  page["items"]
    .as_array()
    .unwrap()
    .iter()
    .map(|item| item["round"].as_str().unwrap().to_owned())
    .collect()
}

#[tokio::test]
async fn a_read_naming_no_round_answers_the_rooms_newest_rounds_oldest_first() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let round = |n: i64| format!("round:{ROOM}:{n}");

  written(
    &scratch,
    &[
      (ROOM, &round(1)),
      (ROOM, &round(2)),
      (-901, "round:-901:1"),
      (ROOM, &round(3)),
      (ROOM, &round(2)),
    ],
  )
  .await;

  let (status, newest) = get(&front, &format!("/rounds/drafts?room={ROOM}&limit=2")).await;

  assert_eq!(status, StatusCode::OK, "{newest}");
  assert_eq!(
    rounds(&newest),
    [round(3), round(2)],
    "the round written to last is the newest: {newest}"
  );
  assert_eq!(newest["more"], true, "an older round stands");
  assert_eq!(newest["items"][1]["rows"].as_array().unwrap().len(), 2, "a round answers all its rows");

  let (_, all) = get(&front, &format!("/rounds/drafts?room={ROOM}")).await;

  assert_eq!(rounds(&all), [round(1), round(3), round(2)]);
  assert_eq!(all["more"], false);

  let (_, named) = get(&front, &format!("/rounds/drafts?room={ROOM}&rounds={}", round(1))).await;

  assert_eq!(rounds(&named), [round(1)], "a read naming rounds answers those alone");
}

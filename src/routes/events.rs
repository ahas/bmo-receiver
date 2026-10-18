//! The event stream, the news a writer outside the receiver brings, the live board and the health check
//! (docs/receiver.md, 「The event stream」). What goes into the stream is the news the receiver makes itself and
//! what `POST /event` says: a message or a briefing checked against the room's record first, and the profile
//! pictures a chat service took.

use axum::{
  body::Bytes,
  extract::{Query, State},
  http::StatusCode,
  response::Response,
};
use serde::Deserialize;
use serde_json::{Value, json};

use receiver_policy::chat_round::CHAT_ROUND;

use sea_orm::DatabaseConnection;

use crate::events;
use crate::one;
use crate::receiver::Receiver;
use crate::routes::{Answered, Refusal, Served, answered, read, refused};
use crate::store::settings;

/// The events a writer outside the receiver may say, in the order the parity fixture keeps them.
pub const TYPES: [&str; 3] = ["message", "briefing", "avatars-updated"];

/// A stream's cursor: the event it was taken against, and the run it was taken in.
#[derive(Deserialize)]
pub struct Streamed {
  #[serde(default)]
  after: Option<String>,

  #[serde(default)]
  run: Option<String>,
}

/// `GET /events`: a screen's stream, from the cursor it was taken against.
pub async fn stream(State(served): State<Served>, query: Query<Streamed>) -> Response {
  let receiver: Receiver = (*served.receiver).clone();
  let after = query.after.as_deref().and_then(leading_whole).unwrap_or(0);

  receiver.news().listen(after, query.run.as_deref())
}

// The whole number a cursor opens on, as `parseInt` reads one: digits after an optional sign, with
// whatever follows them let go.
pub fn leading_whole(text: &str) -> Option<i64> {
  let text = text.trim_start();
  let sign = usize::from(text.starts_with(['-', '+']));
  let digits = text[sign..].chars().take_while(char::is_ascii_digit).count();

  text[..sign + digits].parse().ok()
}

/// `GET /live`: the picture of what is running, asked for once.
pub async fn live(State(served): State<Served>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  answered(receiver.news().picture())
}

/// `GET /health`: that the receiver is up — this process and the moment it started, which the deployment's
/// wait matches against the unit — whose chat is the owner's, and its version.
pub async fn health(State(served): State<Served>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  answered(json!({
    "ok": true,
    "ownerId": receiver.owner_id(),
    "bot": { "pid": std::process::id(), "since": receiver.news().started() },
    "version": crate::VERSION,
  }))
}

/// The news a writer outside the receiver brings.
pub async fn told(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<Value>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let kind = match asked.get("type") {
    None => "undefined".to_owned(),
    Some(Value::String(kind)) => kind.clone(),
    Some(other) => other.to_string(),
  };
  let who = events::relay_caller(asked.get("who").and_then(Value::as_str));

  match kind.as_str() {
    "message" => message(&receiver, &asked, &who).await,
    "briefing" => briefing(&receiver, &asked, &who).await,
    "avatars-updated" => {
      receiver.news().publish(&events::avatars());

      answered(json!({ "ok": true }))
    }
    kind => Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      &format!("Unknown event {}; this route takes {}.", kind, TYPES.join(", ")),
    )),
  }
}

fn bad(error: &str) -> Refusal {
  refused(StatusCode::BAD_REQUEST, "bad-request", error)
}

// A whole number as `Number.isSafeInteger` takes one.
fn number(body: &Value, key: &str) -> Option<i64> {
  body.get(key).and_then(Value::as_i64).filter(|whole| whole.unsigned_abs() < 1 << 53)
}

// A message event names its room and its message, and the row must stand: the event is news of a row,
// and a row nobody can read is no news.
async fn message(receiver: &Receiver, body: &Value, who: &str) -> Answered {
  let (Some(chat_id), Some(message_id)) = (number(body, "chatId"), number(body, "messageId")) else {
    return Err(bad("a message event names its chatId and messageId as integers"));
  };

  held(receiver.db(), chat_id, message_id).await?;
  receiver
    .news()
    .publish(&json!({ "type": "message", "chatId": chat_id, "messageId": message_id, "detail": who }));

  answered(json!({ "ok": true }))
}

// A voice briefing a runner put in a room, for every listener to play (docs/voiced-notices.md, 「What a
// listener hears」), stamped with whether it plays by itself as the owner's switch stands now.
async fn briefing(receiver: &Receiver, body: &Value, who: &str) -> Answered {
  let (Some(chat_id), Some(message_id)) = (number(body, "chatId"), number(body, "messageId")) else {
    return Err(bad("a briefing event names its chatId and messageId as integers"));
  };
  let seconds = body.get("seconds").unwrap_or(&Value::Null);
  let seconds_fit =
    seconds.is_null() || seconds.as_f64().is_some_and(|held| held.is_finite() && held >= 0.0);
  let (Some(text), Some(sent_at), true) = (
    body.get("text").and_then(Value::as_str),
    number(body, "sentAt"),
    seconds_fit && body.get("seconds").is_some(),
  ) else {
    return Err(bad(
      "a briefing event carries its text, its seconds (a number or null) and when it was sent",
    ));
  };
  let emotions = &CHAT_ROUND.voice.emotions;
  let emotion = body.get("emotion");

  if emotion
    .is_some_and(|emotion| !emotion.as_str().is_some_and(|word| emotions.iter().any(|one| one == word)))
  {
    return Err(bad("a briefing event's emotion is one of the briefing emotions, or left out"));
  }

  let title = body.get("title");

  if title.is_some_and(|title| !title.as_str().is_some_and(|line| !line.trim().is_empty())) {
    return Err(bad("a briefing event's title is a line of text, or left out"));
  }

  let expression = body.get("expression");

  if expression.is_some_and(|expression| !expression.as_str().is_some_and(|word| !word.trim().is_empty())) {
    return Err(bad("a briefing event's expression is a word, or left out"));
  }

  held(receiver.db(), chat_id, message_id).await?;

  let mut heard = json!({
    "text": text,
    "seconds": seconds.clone(),
    "sentAt": sent_at,
    "autoplay": !autoplay_off(receiver.db()).await,
  });

  for (key, held) in [("emotion", emotion), ("title", title), ("expression", expression)] {
    if let Some(held) = held {
      heard[key] = held.clone();
    }
  }

  receiver.news().publish(&json!({
    "type": "briefing",
    "chatId": chat_id,
    "messageId": message_id,
    "detail": who,
    "briefing": heard,
  }));

  answered(json!({ "ok": true }))
}

// A unit outside the receiver saved a message it sent: the row must stand in the room's record.
async fn held(db: &DatabaseConnection, chat_id: i64, message_id: i64) -> Result<(), Refusal> {
  let row = one(
    db,
    r#"SELECT 1 AS "one" FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2"#,
    vec![chat_id.into(), message_id.into()],
  )
  .await
  .map_err(|_| bad("the room's record could not be read"))?;

  match row {
    Some(_) => Ok(()),
    None => Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("No message {message_id} in chat {chat_id}."),
    )),
  }
}

// Whether the briefing plays by itself: the owner's switch, on unless it holds "off".
async fn autoplay_off(db: &DatabaseConnection) -> bool {
  settings::get(db, "voice_briefing_autoplay").await.ok().flatten().as_deref() == Some("off")
}

//! Words and files the owner typed into a room on the screen (docs/boundary-api.md, 「The owner's words」;
//! docs/receiver.md, 「The owner's words」): his private chat answers where it was spoken to, so his words there
//! are kept under a console number and handed to that chat's conversation, and the files he puts in there as much
//! as his words; any other room takes them as the owner, on the road the room declares for a line nobody asked
//! for. A room the receiver does not hold is refused before anything is written.

use axum::{
  body::Bytes,
  extract::{Multipart, State},
  http::StatusCode,
};
use bmo_chat_client::MessageFile;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Answered, Refusal, Served, answered, read, refused, store};
use crate::receiver::{Context, Inbound, NotReceived, Receiver, rows};
use crate::roads::{Road, record};
use crate::routes::as_owner::{OwnerFile, caption_limit, refusal, say as owner_say, send_files};
use crate::{now_ms, one};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SayBody {
  chat_id: i64,
  text: String,

  #[serde(default)]
  reply_to: Option<Value>,
}

// A number written as words is read as far as its digits go, the way a browser's parseInt reads one.
fn leading_integer(text: &str) -> Option<i64> {
  let text = text.trim_start();
  let (sign, digits) = match text.strip_prefix('-') {
    Some(rest) => (-1, rest),
    None => (1, text.strip_prefix('+').unwrap_or(text)),
  };
  let digits = digits.chars().take_while(char::is_ascii_digit).collect::<String>();

  digits.parse::<i64>().ok().and_then(|number| number.checked_mul(sign))
}

// Whether the room is one the receiver serves: the owner's own, or one its record holds a title or a message for.
pub(crate) async fn known(receiver: &Receiver, chat_id: i64) -> Result<bool, Refusal> {
  if chat_id == receiver.owner_id() {
    return Ok(true);
  }

  let titled = one(
    receiver.db(),
    r#"SELECT 1 AS "one" FROM "chat_titles" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await
  .map_err(store)?;
  let spoken = one(
    receiver.db(),
    r#"SELECT 1 AS "one" FROM "messages" WHERE "chat_id" = $1 LIMIT 1"#,
    vec![chat_id.into()],
  )
  .await
  .map_err(store)?;

  Ok(titled.is_some() || spoken.is_some())
}

// The message the words answer, where they name one: refused where they name one the room does not hold.
async fn reply_target(
  receiver: &Receiver,
  chat_id: i64,
  value: Option<&Value>,
) -> Result<Option<i64>, Refusal> {
  let number = match value {
    None | Some(Value::Null) => return Ok(None),
    Some(Value::String(text)) if text.is_empty() => return Ok(None),
    Some(Value::Number(number)) => number.as_i64(),
    Some(Value::String(text)) => leading_integer(text),
    Some(other) => leading_integer(&other.to_string()),
  };
  let Some(message_id) = number.filter(|number| *number != 0) else {
    return Err(refused(StatusCode::CONFLICT, "reply-missing", "replyTo must be a message number"));
  };

  if rows::message(receiver.db(), chat_id, message_id, Road::Record.key())
    .await
    .map_err(store)?
    .is_none()
  {
    return Err(refused(
      StatusCode::CONFLICT,
      "reply-missing",
      &format!("the room holds no message {message_id}, so nothing is answered"),
    ));
  }

  Ok(Some(message_id))
}

// Why a call the receiver's own lines needed failed, in the receiver's error shape.
fn failed(why: NotReceived) -> Refusal {
  match why {
    NotReceived::Store(why) => store(why),
    why => refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string()),
  }
}

pub async fn say(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<SayBody>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let text = asked.text.trim();

  if text.is_empty() {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      "chatId (number) and text (non-empty string) are required",
    ));
  }

  if !known(&receiver, asked.chat_id).await? {
    return Err(refused(StatusCode::NOT_FOUND, "no-route", &format!("unknown chat {}", asked.chat_id)));
  }

  let reply = reply_target(&receiver, asked.chat_id, asked.reply_to.as_ref()).await?;

  if asked.chat_id == receiver.owner_id() {
    let message_id = record::next_number(receiver.db(), asked.chat_id).await.map_err(store)?;
    let message = Inbound {
      chat_id: asked.chat_id,
      message_id,
      sender_id: Some(receiver.owner_id()),
      sender_name: receiver.deployment().sender_name.clone(),
      sent_at: now_ms() / 1000,
      text: text.to_owned(),
      image_path: None,
      reply_to_message_id: reply,
      via: Road::Record.key().to_owned(),
      mode: None,
      via_reader: None,
      album: None,
    };

    Receiver::message(
      &receiver,
      &message,
      &Context::default(),
      receiver.roads().room_facts(asked.chat_id).await,
    )
    .await
    .map_err(failed)?;

    return answered(json!({ "id": message_id }));
  }

  let message_id = owner_say(&receiver, asked.chat_id, text, reply).await?;

  answered(json!({ "id": message_id }))
}

pub async fn say_file(State(served): State<Served>, form: Multipart) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let mut form = form;
  let mut chat_id: Option<i64> = None;
  let mut caption = String::new();
  let mut reply_field: Option<String> = None;
  let mut given: Vec<OwnerFile> = vec![];

  while let Some(field) = form
    .next_field()
    .await
    .map_err(|why| refused(StatusCode::BAD_REQUEST, "bad-request", &why.body_text()))?
  {
    let name = field.name().unwrap_or_default();

    match name {
      "chatId" => {
        let written = field
          .text()
          .await
          .map_err(|why| refused(StatusCode::BAD_REQUEST, "bad-request", &why.body_text()))?;

        chat_id = leading_integer(&written);
      }
      "text" | "caption" => {
        caption = field
          .text()
          .await
          .map_err(|why| refused(StatusCode::BAD_REQUEST, "bad-request", &why.body_text()))?
      }
      "replyTo" => {
        reply_field = Some(
          field
            .text()
            .await
            .map_err(|why| refused(StatusCode::BAD_REQUEST, "bad-request", &why.body_text()))?,
        )
      }
      "file" => {
        let name = field.file_name().unwrap_or_default().to_owned();
        let mime = field.content_type().unwrap_or_default().to_owned();
        let bytes = field
          .bytes()
          .await
          .map_err(|why| refused(StatusCode::BAD_REQUEST, "bad-request", &why.body_text()))?;

        given.push(OwnerFile {
          name,
          mime,
          bytes: bytes.to_vec(),
        });
      }
      _ => {}
    }
  }

  let Some(chat_id) = chat_id else {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "chatId (number) and file are required"));
  };

  if given.is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "chatId (number) and file are required"));
  }

  if !known(&receiver, chat_id).await? {
    return Err(refused(StatusCode::NOT_FOUND, "no-route", &format!("unknown chat {chat_id}")));
  }

  let reply_value = reply_field.map(Value::String);
  let reply = reply_target(&receiver, chat_id, reply_value.as_ref()).await?;
  let mut turned_away: Vec<Value> = vec![];
  let mut kept: Vec<OwnerFile> = vec![];

  for file in given {
    match refusal(&file.name, u64::try_from(file.bytes.len()).unwrap_or(0)) {
      Some(reason) => turned_away.push(json!({ "name": file.name, "reason": reason })),
      None => kept.push(file.clone()),
    }
  }

  if kept.is_empty() {
    let why = turned_away
      .first()
      .map(|held| held.get("reason").and_then(Value::as_str).unwrap_or_default().to_owned())
      .unwrap_or("no file stood".to_owned());

    return Err(refused(StatusCode::PAYLOAD_TOO_LARGE, "too-large", &why));
  }

  if chat_id == receiver.owner_id() {
    let mut sent: Vec<Value> = vec![];

    for (at, file) in kept.iter().enumerate() {
      let message_id = record::next_number(receiver.db(), chat_id).await.map_err(store)?;
      let message = Inbound {
        chat_id,
        message_id,
        sender_id: Some(receiver.owner_id()),
        sender_name: receiver.deployment().sender_name.clone(),
        sent_at: now_ms() / 1000,
        text: caption.clone(),
        image_path: None,
        reply_to_message_id: if at == 0 { reply } else { None },
        via: Road::Record.key().to_owned(),
        mode: None,
        via_reader: None,
        album: None,
      };
      // The record hands the file over as its own message's one file, identified by that place.
      let item = MessageFile {
        id: "1".to_owned(),
        name: file.name.clone(),
        mime: file.mime.clone(),
        bytes: file.bytes.clone(),
        kind: None,
      };
      let room = receiver.roads().room_facts(chat_id).await;

      Receiver::attachment(
        &receiver,
        &message,
        &Context {
          carries_file: true,
          ..Context::default()
        },
        room,
        &[item],
      )
      .await
      .map_err(failed)?;
      sent.push(json!({ "name": file.name, "id": message_id }));
    }

    return answered(
      json!({ "id": sent.first().and_then(|held| held.get("id")), "sent": sent, "checking": [], "failed": turned_away }),
    );
  }

  let long = caption_limit(&receiver, chat_id)
    .await?
    .is_some_and(|limit| caption.encode_utf16().count() > usize::try_from(limit).unwrap_or(0));

  if long {
    owner_say(&receiver, chat_id, &caption, reply).await?;
  }

  let words = if long { String::new() } else { caption };
  let mut outcome = send_files(&receiver, chat_id, &kept, &words, if long { None } else { reply }).await?;

  if outcome.sent.is_empty() && outcome.checking.is_empty() {
    let why = outcome.refusal.unwrap_or("the road took none of the files".to_owned());

    return Err(refused(StatusCode::CONFLICT, "not-sent", &why));
  }

  turned_away.append(&mut outcome.failed);

  answered(json!({
    "id": outcome.sent.first().and_then(|held| held.get("id")),
    "sent": outcome.sent,
    "checking": outcome.checking,
    "failed": turned_away,
  }))
}

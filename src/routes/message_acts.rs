//! A signed-in person's acts on a room's message from the console (docs/api.md, 「Messages」; docs/messages.md,
//! 「What an agent can send back」): a reaction of their own, a pin, an edit and a delete of their own message, the
//! administrator's delete of any, a sticker of the record's set, and the room's pinned messages. The record keeps
//! each act and tells `message.changed`; the owner's acts go on in his own voice where the message stands on the
//! room's other providers, and a member's stand on the record alone.

use axum::{
  Json,
  body::Bytes,
  extract::{Path, State},
  http::{HeaderMap, StatusCode},
};
use bmo_chat_client::{ChatSend, ChatVoice};
use receiver_codes::MessageActRefusal;
use receiver_policy::roads::ROADS;
use serde::Deserialize;
use serde_json::{Value, json};

use super::rooms::message_shape;
use super::{Answered, Refusal, Served, answered, code, read, refused, store};
use crate::all;
use crate::receiver::{Receiver, reads, rows};
use crate::roads::providers::{self, Carried, Delivery};
use crate::roads::{Road, record};

/// Who the console relay names: the number the person acts under in the room, every sender the web unit links to
/// them, whether they are the owner and whether they are the administrator.
struct Caller {
  number: i64,
  senders: Vec<i64>,
  owner: bool,
  administrator: bool,
}

impl Caller {
  // Whether one of the caller's senders said the message.
  fn said(&self, kept: &reads::Page) -> bool {
    kept.sender_id.is_some_and(|sender| self.senders.contains(&sender))
  }

  // The voice the caller's act goes on in to the room's other providers: the owner's own, and none for a member,
  // whose act stands on the record alone.
  fn voice(&self) -> Option<ChatVoice> {
    self.owner.then_some(ChatVoice::HumanReader)
  }
}

// The caller the relay's `Owner-Key` and `Owner-Role` name: the user's sender that stands in the room, else the
// first of the user's senders.
async fn caller(receiver: &Receiver, asked: &HeaderMap, room: i64) -> Result<Caller, Refusal> {
  let header =
    |name: &str| asked.get(name).and_then(|value| value.to_str().ok()).map(str::trim).unwrap_or_default();
  let user = header("owner-key");

  if user.is_empty() {
    return Err(code(&MessageActRefusal::NoCaller));
  }

  let senders = receiver.units().web.senders_of_user(user).await.unwrap_or_default();
  let Some(first) = senders.first().copied() else {
    return Err(code(&MessageActRefusal::NoCaller));
  };
  let standing = all(
    receiver.db(),
    r#"SELECT DISTINCT "sender_id" FROM "messages" WHERE "chat_id" = $1 AND "sender_id" IS NOT NULL"#,
    vec![room.into()],
  )
  .await
  .map_err(store)?
  .iter()
  .filter_map(|row| row.try_get::<i64>("", "sender_id").ok())
  .collect::<Vec<i64>>();
  let number = senders.iter().copied().find(|sender| standing.contains(sender)).unwrap_or(first);

  Ok(Caller {
    number,
    owner: senders.contains(&receiver.owner_id()),
    senders,
    administrator: header("owner-role") == "administrator",
  })
}

// The message as the record keeps it, refused where the room keeps none or it was deleted.
async fn standing(receiver: &Receiver, room: i64, message: i64) -> Result<reads::Page, Refusal> {
  let Some(kept) = reads::message_of(receiver.db(), room, message).await.map_err(store)? else {
    return Err(code(&MessageActRefusal::MessageUnknown { room, message }));
  };

  if kept.deleted_at.is_some() {
    return Err(code(&MessageActRefusal::MessageDeleted { room, message }));
  }

  Ok(kept)
}

// The act kept and told, and carried in the caller's voice where they have one; the message answered as it now
// stands.
async fn acted(
  receiver: &Receiver,
  by: &Caller,
  (room, message): (i64, i64),
  act: record::Act<'_>,
  send: ChatSend,
) -> Answered {
  record::act(receiver.db(), room, message, act).await.map_err(store)?;
  receiver.news().publish(&crate::events::message_changed(room, message));

  if let Some(voice) = by.voice() {
    providers::carry_act(receiver.db(), receiver.roads(), room, message, &send, Some(voice)).await;
  }

  let shaped = message_shape(receiver, room, message).await.map_err(store)?;

  answered(shaped.unwrap_or(Value::Null))
}

#[derive(Deserialize)]
pub struct Emoji {
  emoji: String,
}

/// The caller's reaction put on a message.
pub async fn react(
  State(served): State<Served>,
  Path((room, message)): Path<(i64, i64)>,
  asked: HeaderMap,
  body: Bytes,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let wanted = read::<Emoji>(&body)?;

  if wanted.emoji.trim().is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "A reaction names an emoji."));
  }

  let by = caller(&receiver, &asked, room).await?;

  standing(&receiver, room, message).await?;

  let reactor = by.number.to_string();
  let send = ChatSend::Reaction {
    message_id: message,
    emoji: wanted.emoji.trim().to_owned(),
  };

  acted(
    &receiver,
    &by,
    (room, message),
    record::Act::Reaction {
      by: &reactor,
      emoji: wanted.emoji.trim(),
    },
    send,
  )
  .await
}

/// The caller's reaction taken off a message.
pub async fn unreact(
  State(served): State<Served>,
  Path((room, message)): Path<(i64, i64)>,
  asked: HeaderMap,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let by = caller(&receiver, &asked, room).await?;

  standing(&receiver, room, message).await?;

  let reactor = by.number.to_string();
  let send = ChatSend::Reaction {
    message_id: message,
    emoji: String::new(),
  };

  acted(
    &receiver,
    &by,
    (room, message),
    record::Act::Reaction {
      by: &reactor,
      emoji: "",
    },
    send,
  )
  .await
}

#[derive(Deserialize)]
pub struct Pinned {
  on: bool,
}

/// A message pinned, or unpinned, by anyone signed in.
pub async fn pin(
  State(served): State<Served>,
  Path((room, message)): Path<(i64, i64)>,
  asked: HeaderMap,
  body: Bytes,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let wanted = read::<Pinned>(&body)?;

  let by = caller(&receiver, &asked, room).await?;

  standing(&receiver, room, message).await?;

  let send = ChatSend::Pin {
    message_id: message,
    on: wanted.on,
    silent: true,
  };

  acted(&receiver, &by, (room, message), record::Act::Pin { on: wanted.on }, send).await
}

#[derive(Deserialize)]
pub struct Edited {
  text: String,
}

/// The caller's own message edited.
pub async fn edit(
  State(served): State<Served>,
  Path((room, message)): Path<(i64, i64)>,
  asked: HeaderMap,
  body: Bytes,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let wanted = read::<Edited>(&body)?;

  if wanted.text.trim().is_empty() {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      "An edit names the message's new words.",
    ));
  }

  let by = caller(&receiver, &asked, room).await?;
  let kept = standing(&receiver, room, message).await?;

  if !by.said(&kept) {
    return Err(code(&MessageActRefusal::MessageForbidden { room, message }));
  }

  let send = ChatSend::Edit {
    message_id: message,
    text: wanted.text.clone(),
  };

  acted(&receiver, &by, (room, message), record::Act::Edit { text: &wanted.text }, send).await
}

/// The caller's own message deleted, or any by the administrator.
pub async fn delete(
  State(served): State<Served>,
  Path((room, message)): Path<(i64, i64)>,
  asked: HeaderMap,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let by = caller(&receiver, &asked, room).await?;
  let kept = standing(&receiver, room, message).await?;

  if !by.said(&kept) && !by.administrator {
    return Err(code(&MessageActRefusal::MessageForbidden { room, message }));
  }

  acted(
    &receiver,
    &by,
    (room, message),
    record::Act::Delete,
    ChatSend::Delete {
      message_id: message,
    },
  )
  .await
}

/// The stickers the record keeps, each by its emoji.
pub async fn stickers() -> Answered {
  let items: Vec<Value> = ROADS.record.stickers.iter().map(|emoji| json!({ "emoji": emoji })).collect();

  answered(json!({ "items": items }))
}

/// A sticker of the record's set sent as the caller: a message of their own, carried in the owner's voice where the
/// caller is the owner and kept on the record alone otherwise.
pub async fn sticker(
  State(served): State<Served>,
  Path(room): Path<i64>,
  asked: HeaderMap,
  body: Bytes,
) -> Result<(StatusCode, Json<Value>), Refusal> {
  let receiver: Receiver = (*served.receiver).clone();
  let wanted = read::<Emoji>(&body)?;
  let by = caller(&receiver, &asked, room).await?;
  let emoji = wanted.emoji.trim().to_owned();

  if !ROADS.record.stickers.contains(&emoji) {
    return Err(code(&MessageActRefusal::UnknownSticker { emoji }));
  }

  let owner = by.owner;
  let name = if owner {
    receiver.owner_name(room).await
  } else {
    let shown = rows::shown_name(receiver.db(), room, by.number, Road::Record.key())
      .await
      .map_err(store)?;

    shown.unwrap_or_else(|| by.number.to_string())
  };
  let kept = record::Kept {
    chat_id: room,
    text: &emoji,
    reply_to: None,
    via: Road::Record.key(),
    sender_id: Some(by.number),
    name: &name,
    agent: None,
    voice: None,
  };
  let message = record::keep(receiver.db(), receiver.news(), kept).await.map_err(store)?;

  record::mark_sticker(receiver.db(), room, message).await.map_err(store)?;

  if owner {
    let delivery = Delivery {
      chat_id: room,
      message_id: message,
      reply_to: None,
      owner: true,
      standing: vec![Road::Record.key().to_owned()],
      carried: Carried::Send(ChatSend::Sticker {
        emoji: emoji.clone(),
      }),
    };

    providers::deliver(receiver.db(), receiver.roads(), &delivery).await;
  }

  let shaped = message_shape(&receiver, room, message).await.map_err(store)?;

  Ok((StatusCode::CREATED, Json(shaped.unwrap_or(Value::Null))))
}

/// The room's pinned messages, newest pin first.
pub async fn pins(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let pinned = all(
    receiver.db(),
    r#"SELECT "message_id" FROM "messages" WHERE "chat_id" = $1 AND "pinned_at" IS NOT NULL AND "deleted_at" IS NULL
       ORDER BY "pinned_at" DESC, "rowid" DESC"#,
    vec![room.into()],
  )
  .await
  .map_err(store)?;
  let mut items: Vec<Value> = vec![];

  for row in pinned {
    let Ok(message) = row.try_get::<i64>("", "message_id") else { continue };

    if let Some(shaped) = message_shape(&receiver, room, message).await.map_err(store)? {
      items.push(shaped);
    }
  }

  answered(json!({ "items": items }))
}

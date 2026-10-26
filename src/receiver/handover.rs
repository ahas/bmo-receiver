//! The hand-overs a chat service makes (docs/api.md, 「The chat services」): a message handed over live, edited or
//! read back, and the files a message's gathering promised, served by bmo-chat-client's receiving-side router and
//! judged by the one receiver. What the service already knows of the room is taken as it says it; a hand-over that
//! says nothing of the room is read off the room's road. What the platform handed over beside a message as its
//! `metadata` is kept as it arrived once the message is judged. A message said live and kept goes on to the room's
//! other providers, and a provider's copy of a message carried to it from the room is that message, held already
//! (docs/receiver.md, 「A room's chat providers」).

use std::sync::Arc;

use async_trait::async_trait;
use bmo_chat_client::{
  AvatarChange, ChatReceiver, ErrorCode, FileItem, GatheredPatch, HandedMessage, HeldMessage,
  InboundChatContext, MessageOrigin, ReceiverRoom, RoomFacts, RoomKind, RoomQuery, RoomsQuery, WireError,
  receiver_router,
};

use super::{Context, Inbound, NotReceived, Receiver, Room, metadata};
use crate::all;
use crate::receiver::reads;
use crate::roads::providers::{self, Carried, Delivery};

/// The message as the receiver keeps it, its moment in epoch milliseconds as the record keeps it.
pub fn inbound(room_id: i64, message_id: i64, message: &HandedMessage) -> Inbound {
  Inbound {
    chat_id: room_id,
    message_id,
    sender_id: message.sender_id,
    sender_name: message.sender_name.clone(),
    sent_at: message.sent_at,
    text: message.text.clone(),
    image_path: None,
    reply_to_message_id: message.reply_to_message_id,
    via: message.via.clone(),
    mode: serde_json::to_value(message.mode)
      .ok()
      .and_then(|mode| mode.as_str().map(str::to_owned)),
    via_reader: None,
    album: message.album,
  }
}

/// What the platform said beside the message, as the receiver reads it.
pub fn context(context: &InboundChatContext) -> Context {
  Context {
    room_title: context.room_title.clone(),
    sender_username: context.sender_username.clone(),
    reply_to_sender_id: context.reply_to_sender_id,
    spoken_text: context.spoken_text.clone(),
    carries_file: context.carries_file,
    gathering: false,
  }
}

/// A file a message carried, as a send carries it on.
pub fn file_item(file: &bmo_chat_client::MessageFile) -> FileItem {
  FileItem {
    name: file.name.clone(),
    mime: file.mime.clone(),
    bytes: file.bytes.clone(),
  }
}

/// The message as the receiving side hands it back once kept.
fn handed(inbound: &Inbound, files: Vec<bmo_chat_client::MessageFile>, room: RoomFacts) -> HandedMessage {
  HandedMessage {
    origin: MessageOrigin::Live,
    sender_id: inbound.sender_id,
    sender_name: inbound.sender_name.clone(),
    sent_at: inbound.sent_at,
    text: inbound.text.clone(),
    reply_to_message_id: inbound.reply_to_message_id,
    via: inbound.via.clone(),
    album: inbound.album,
    mode: serde_json::from_value(serde_json::json!(
      inbound.mode.clone().unwrap_or_else(|| "direct".to_owned())
    ))
    .unwrap_or(bmo_chat_client::ChatVoice::Direct),
    context: InboundChatContext::default(),
    room,
    files,
    gathering: false,
    gather_failure: None,
  }
}

impl Receiver {
  // The room as the hand-over says it, or as its road says it where the hand-over says nothing.
  async fn room_of(&self, room_id: i64, facts: RoomFacts) -> Room {
    if facts == RoomFacts::default() {
      return self.roads().room_facts(room_id).await;
    }

    Room {
      owner_room: self.roads().owners_room(room_id, facts.kind == Some(RoomKind::Owner)),
      identity: facts.identity,
      private_to: facts.private_to,
    }
  }
}

fn failed(why: NotReceived) -> WireError {
  WireError::new(ErrorCode::Failed, format!("The hand-over was not taken: {why}."))
}

#[async_trait]
impl ChatReceiver for Receiver {
  async fn hand_over(
    &self,
    room_id: i64,
    message_id: i64,
    message: HandedMessage,
  ) -> Result<HeldMessage, WireError> {
    // A provider's copy of a message the room already keeps, carried to it from the room, is that message.
    let copy = !matches!(message.origin, MessageOrigin::Edit)
      && providers::echo(self.db(), room_id, &message.via, message_id, message.context.tag.as_deref())
        .await
        .map_err(|why| failed(NotReceived::Store(why)))?;

    if copy {
      return Ok(HeldMessage {
        created: false,
        message,
      });
    }

    let held = rows_holds(self, room_id, message_id, &message.via).await.map_err(failed)?;
    let room = self.room_of(room_id, message.room).await;
    let said = inbound(room_id, message_id, &message);
    let mut seen = context(&message.context);
    seen.gathering = message.gathering;

    match message.origin {
      MessageOrigin::History => {
        Receiver::history(self, &said, &seen, room).await.map_err(failed)?;
      }
      MessageOrigin::Edit => {
        Receiver::edited(self, &said).await.map_err(failed)?;
      }
      MessageOrigin::Live if message.gathering || !message.files.is_empty() => {
        Receiver::attachment(self, &said, &seen, room, &message.files).await.map_err(failed)?;
      }
      MessageOrigin::Live => {
        Receiver::message(self, &said, &seen, room).await.map_err(failed)?;
      }
    }

    if let Some(metadata) = message.context.metadata.as_ref() {
      metadata::keep(self.db(), room_id, message_id, metadata)
        .await
        .map_err(|why| failed(NotReceived::Store(why)))?;
    }

    // A message said live and kept now goes to the room's other providers; one read back from the past, or one
    // the room already held, goes nowhere.
    if matches!(message.origin, MessageOrigin::Live)
      && !held
      && rows_holds(self, room_id, message_id, &message.via).await.map_err(failed)?
    {
      let carried = if message.files.is_empty() {
        (!message.gathering).then(|| Carried::words(seen.spoken_text.as_deref().unwrap_or(&said.text)))
      } else {
        Some(Carried::Files {
          files: message.files.iter().map(file_item).collect(),
          caption: seen.spoken_text.clone().unwrap_or_default(),
        })
      };

      if let Some(carried) = carried {
        let delivery = Delivery {
          chat_id: room_id,
          message_id,
          reply_to: said.reply_to_message_id,
          owner: said.sender_id == Some(self.owner_id()),
          standing: vec![said.via.clone()],
          carried,
        };

        providers::deliver(self.db(), self.roads(), &delivery).await;
      }
    }

    Ok(HeldMessage {
      created: matches!(message.origin, MessageOrigin::Live) && !held,
      message,
    })
  }

  async fn gathered(
    &self,
    room_id: i64,
    message_id: i64,
    patch: GatheredPatch,
  ) -> Result<HandedMessage, WireError> {
    let Some(inbound) = rows_message(self, room_id, message_id).await.map_err(failed)? else {
      return Err(WireError::new(
        ErrorCode::NotFound,
        format!("The receiver keeps no message {message_id} in room {room_id}."),
      ));
    };
    Receiver::gathered(self, room_id, message_id, &inbound.via, &patch)
      .await
      .map_err(failed)?;

    // The files a gathering promised go to the room's other providers once they stand.
    if patch.gather_failure.is_none() && !patch.files.is_empty() {
      let delivery = Delivery {
        chat_id: room_id,
        message_id,
        reply_to: inbound.reply_to_message_id,
        owner: inbound.sender_id == Some(self.owner_id()),
        standing: vec![inbound.via.clone()],
        carried: Carried::Files {
          files: patch.files.iter().map(file_item).collect(),
          caption: String::new(),
        },
      };

      providers::deliver(self.db(), self.roads(), &delivery).await;
    }
    let room = self.roads().room_facts(room_id).await;

    Ok(handed(
      &inbound,
      patch.files,
      RoomFacts {
        kind: Some(if room.owner_room { RoomKind::Owner } else { RoomKind::Group }),
        identity: room.identity,
        private_to: room.private_to,
      },
    ))
  }

  async fn avatars_changed(&self, _change: AvatarChange) -> Result<(), WireError> {
    self.news().publish(&crate::events::avatars());

    Ok(())
  }

  async fn room(&self, room_id: i64, query: RoomQuery) -> Result<Option<ReceiverRoom>, WireError> {
    let room = self.roads().room_facts(room_id).await;
    let held = reads::oldest(self.db(), room_id)
      .await
      .map_err(|why| WireError::new(ErrorCode::Failed, format!("The room could not be read: {why}.")))?;
    let listened = Receiver::listened(room_id, room);
    // The people's words of a read-only room added on the asking service are its to read (docs/receiver.md,
    // 「A read-only room」).
    let read = providers::read_asked(self.db(), self.roads(), room_id, &query.via)
      .await
      .map_err(|why| WireError::new(ErrorCode::Failed, format!("The room could not be read: {why}.")))?;
    let Some((message_id, sent_at)) = held else {
      return Ok((listened || read).then_some(ReceiverRoom {
        id: room_id,
        listened,
        read,
        oldest_message_id: None,
        oldest_sent_at: None,
      }));
    };

    Ok(Some(ReceiverRoom {
      id: room_id,
      listened,
      read,
      oldest_message_id: Some(message_id),
      oldest_sent_at: Some(sent_at),
    }))
  }

  async fn rooms(&self, query: RoomsQuery) -> Result<Vec<ReceiverRoom>, WireError> {
    let rooms: Vec<i64> = all(
      self.db(),
      r#"SELECT DISTINCT "chat_id" FROM "messages" WHERE "via" = $1
         AND "chat_id" > $2 ORDER BY "chat_id""#,
      vec![query.via.clone().into(), query.after.unwrap_or(i64::MIN).into()],
    )
    .await
    .map_err(|why| WireError::new(ErrorCode::Failed, format!("The rooms could not be read: {why}.")))?
    .iter()
    .map(|row| row.try_get::<i64>("", "chat_id").unwrap_or(0))
    .collect();
    let mut answered: Vec<ReceiverRoom> = vec![];

    for room_id in rooms {
      if let Some(limit) = query.limit
        && answered.len() >= usize::try_from(limit).unwrap_or(usize::MAX)
      {
        break;
      }

      if let Some(room) = ChatReceiver::room(
        self,
        room_id,
        RoomQuery {
          via: query.via.clone(),
        },
      )
      .await?
      {
        answered.push(room);
      }
    }

    Ok(answered)
  }
}

// Whether the record already holds this message on its road.
async fn rows_holds(
  receiver: &Receiver,
  room_id: i64,
  message_id: i64,
  via: &str,
) -> Result<bool, NotReceived> {
  Ok(crate::receiver::rows::message(receiver.db(), room_id, message_id, via).await?.is_some())
}

// The kept row, as the record holds it.
async fn rows_message(
  receiver: &Receiver,
  room_id: i64,
  message_id: i64,
) -> Result<Option<Inbound>, NotReceived> {
  Ok(crate::receiver::rows::message(receiver.db(), room_id, message_id, "").await?)
}

/// The chat services' receiving-side routes over this receiver, answering only a request that carries the
/// receiver's token. They stand under `/chat`, so a provider's base address names that prefix and the
/// receiver's own `/rooms` stays its own.
pub fn routes(receiver: Receiver, token: impl Into<String>) -> axum::Router {
  axum::Router::new().nest("/chat", receiver_router(Arc::new(receiver), token))
}

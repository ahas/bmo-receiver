//! The hand-overs a chat service makes (docs/boundary-api.md, 「For the chat services」): a message, a message
//! carrying a file, an edit and a file gathered, served by bmo-chat-client's receiver routes and judged by the one
//! receiver. What the service already knows of the room is taken as it says it; a hand-over that says nothing of
//! the room is read off the room's road. What the platform handed over beside a message as its `metadata` is kept as
//! it arrived once the message is judged.

use std::sync::Arc;

use async_trait::async_trait;
use bmo_chat_client::{
  ChatGathered, ChatHandover, ChatMessage, ChatReceiver, ErrorCode, InboundChatContext, RoomKind,
  WireError, receiver_router,
};

use super::{Context, Inbound, NotReceived, Receiver, Room, metadata};

/// The message as the receiver keeps it.
pub fn inbound(message: &ChatMessage) -> Inbound {
  Inbound {
    chat_id: message.chat_id,
    message_id: message.message_id,
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
pub fn context(context: Option<&InboundChatContext>) -> Context {
  let Some(context) = context else { return Context::default() };

  Context {
    room_title: context.room_title.clone(),
    sender_username: context.sender_username.clone(),
    reply_to_sender_id: context.reply_to_sender_id,
    spoken_text: context.spoken_text.clone(),
    carries_file: context.carries_file,
    gathering: context.gathering,
  }
}

impl Receiver {
  // The room as the hand-over says it, or as its road says it where the hand-over says nothing.
  async fn room_of(&self, handover: &ChatHandover) -> Room {
    match handover.room {
      Some(facts) => Room {
        owner_room: facts.kind == Some(RoomKind::Owner),
        identity: facts.identity,
        private_to: facts.private_to,
      },
      None => self.roads().room_facts(handover.message.chat_id).await,
    }
  }

  // What the platform handed over beside the message, kept as it arrived once the message is judged; a hand-over
  // carrying none leaves what was kept.
  async fn metadata_of(&self, handover: &ChatHandover) -> Result<(), NotReceived> {
    let Some(metadata) = handover.context.as_ref().and_then(|context| context.metadata.as_ref()) else {
      return Ok(());
    };

    Ok(metadata::keep(self.db(), handover.message.chat_id, handover.message.message_id, metadata).await?)
  }
}

fn failed(why: NotReceived) -> WireError {
  WireError::new(ErrorCode::Failed, format!("The hand-over was not taken: {why}."))
}

#[async_trait]
impl ChatReceiver for Receiver {
  async fn message(&self, handover: ChatHandover) -> Result<(), WireError> {
    let room = self.room_of(&handover).await;

    Receiver::message(self, &inbound(&handover.message), &context(handover.context.as_ref()), room)
      .await
      .map_err(failed)?;
    self.metadata_of(&handover).await.map_err(failed)
  }

  async fn attachment(&self, handover: ChatHandover) -> Result<(), WireError> {
    let room = self.room_of(&handover).await;
    let said = inbound(&handover.message);

    Receiver::attachment(self, &said, &context(handover.context.as_ref()), room, &handover.files)
      .await
      .map_err(failed)?;
    self.metadata_of(&handover).await.map_err(failed)
  }

  async fn edited(&self, handover: ChatHandover) -> Result<(), WireError> {
    Receiver::edited(self, &inbound(&handover.message)).await.map_err(failed)?;
    self.metadata_of(&handover).await.map_err(failed)
  }

  async fn gathered(&self, gathered: ChatGathered) -> Result<(), WireError> {
    Receiver::gathered(self, gathered.chat_id, gathered.message_id, &gathered.via, &gathered.outcome)
      .await
      .map_err(failed)
  }
}

/// The chat services' receiver routes over this receiver, answering only a request that carries the receiver's
/// token.
pub fn routes(receiver: Receiver, token: impl Into<String>) -> axum::Router {
  receiver_router(Arc::new(receiver), token)
}

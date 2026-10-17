//! The chat services' port: every provider route of bmo-chat's `docs/protocol.md`, reached through
//! `bmo-chat-client` (`docs/boundary-api.md`, 「The chat services」). The receiver holds one client for each chat
//! service its configuration lists, by the road's name, and no code of any one of them: what differs between them
//! is read from what each declares.

use std::collections::BTreeMap;
use std::sync::Arc;

use bmo_chat_client::{
  ChatDeclaration, ChatHealth, ChatMediaRequest, ChatMediaResult, ChatProviderClient, ChatSendRequest,
  ChatSendResult, ClientFailure, DialectChecked, DialectReading, RoomKind,
};
use futures_util::future::BoxFuture;

use crate::http::Peer;

/// What a call on a chat service comes back with.
pub type ChatAnswer<T> = Result<T, ClientFailure>;

/// Every call the receiver makes on one chat service.
pub trait Chat: Send + Sync {
  fn declaration(&self) -> BoxFuture<'_, ChatAnswer<ChatDeclaration>>;

  fn health(&self) -> BoxFuture<'_, ChatAnswer<ChatHealth>>;

  /// A send; one the service does not perform comes back as its refusal.
  fn send<'a>(&'a self, request: &'a ChatSendRequest) -> BoxFuture<'a, ChatAnswer<ChatSendResult>>;

  fn media<'a>(&'a self, request: &'a ChatMediaRequest) -> BoxFuture<'a, ChatAnswer<ChatMediaResult>>;

  fn dialect_read<'a>(&'a self, text: &'a str) -> BoxFuture<'a, ChatAnswer<DialectReading>>;

  fn dialect_check<'a>(&'a self, markup: &'a str) -> BoxFuture<'a, ChatAnswer<DialectChecked>>;

  /// Whether the room is the service's.
  fn room_owns(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<bool>>;

  /// The sender number the agent's own words carry in the room.
  fn room_identity(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<i64>>>;

  /// Whether the room is the owner's or a group.
  fn room_kind(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<RoomKind>>;

  /// The one person in a one-to-one with the agent.
  fn room_private_of(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<i64>>>;

  fn room_title(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<String>>>;

  /// The room the owner is reached in unprompted.
  fn owner_room(&self) -> BoxFuture<'_, ChatAnswer<Option<i64>>>;
}

impl Chat for ChatProviderClient {
  fn declaration(&self) -> BoxFuture<'_, ChatAnswer<ChatDeclaration>> {
    Box::pin(ChatProviderClient::declaration(self))
  }

  fn health(&self) -> BoxFuture<'_, ChatAnswer<ChatHealth>> {
    Box::pin(ChatProviderClient::health(self))
  }

  fn send<'a>(&'a self, request: &'a ChatSendRequest) -> BoxFuture<'a, ChatAnswer<ChatSendResult>> {
    Box::pin(ChatProviderClient::send(self, request))
  }

  fn media<'a>(&'a self, request: &'a ChatMediaRequest) -> BoxFuture<'a, ChatAnswer<ChatMediaResult>> {
    Box::pin(ChatProviderClient::media(self, request))
  }

  fn dialect_read<'a>(&'a self, text: &'a str) -> BoxFuture<'a, ChatAnswer<DialectReading>> {
    Box::pin(ChatProviderClient::dialect_read(self, text))
  }

  fn dialect_check<'a>(&'a self, markup: &'a str) -> BoxFuture<'a, ChatAnswer<DialectChecked>> {
    Box::pin(ChatProviderClient::dialect_check(self, markup))
  }

  fn room_owns(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<bool>> {
    Box::pin(ChatProviderClient::room_owns(self, chat_id))
  }

  fn room_identity(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    Box::pin(ChatProviderClient::room_identity(self, chat_id))
  }

  fn room_kind(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<RoomKind>> {
    Box::pin(ChatProviderClient::room_kind(self, chat_id))
  }

  fn room_private_of(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    Box::pin(ChatProviderClient::room_private_of(self, chat_id))
  }

  fn room_title(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<String>>> {
    Box::pin(ChatProviderClient::room_title(self, chat_id))
  }

  fn owner_room(&self) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    Box::pin(ChatProviderClient::owner_room(self))
  }
}

/// The chat services the configuration lists, each by the name of its road.
#[derive(Clone, Default)]
pub struct ChatServices {
  roads: BTreeMap<String, Arc<dyn Chat>>,
}

impl ChatServices {
  /// A client of each chat service at the address and behind the token its road's peer names.
  pub fn from_peers(peers: &BTreeMap<String, Peer>) -> Self {
    Self {
      roads: peers
        .iter()
        .map(|(road, peer)| {
          let chat: Arc<dyn Chat> =
            Arc::new(ChatProviderClient::new(peer.base.clone(), peer.token.clone()));

          (road.clone(), chat)
        })
        .collect(),
    }
  }

  /// The chat services given, each by its road's name.
  pub fn of(roads: impl IntoIterator<Item = (String, Arc<dyn Chat>)>) -> Self {
    Self {
      roads: roads.into_iter().collect(),
    }
  }

  /// The chat service of a road, where the configuration lists it.
  pub fn road(&self, name: &str) -> Option<&Arc<dyn Chat>> {
    self.roads.get(name)
  }

  /// Every road's name, in order.
  pub fn names(&self) -> impl Iterator<Item = &str> {
    self.roads.keys().map(String::as_str)
  }
}

//! The chat services' port: every provider route of bmo-chat's `docs/protocol.md`, reached through
//! `bmo-chat-client` (`docs/boundary-api.md`, 「The chat services」). The receiver holds one client for each chat
//! service its configuration lists, by the road's name, and no code of any one of them: what differs between them
//! is read from what each declares.

use std::collections::BTreeMap;
use std::sync::Arc;

use bmo_chat_client::{
  AccountRoomsQuery, ChatContact, ChatDeclaration, ChatHealth, ChatMediaRequest, ChatProviderClient,
  ChatSendRequest, ChatSent, ChatStart, ClientFailure, ContactsQuery, DialectChecked, DialectReading,
  MessageFile, Page, ProviderRoom, RoomKind,
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
  fn send<'a>(&'a self, request: &'a ChatSendRequest) -> BoxFuture<'a, ChatAnswer<ChatSent>>;

  /// The files and pictures a message carried, fetched from the service again.
  fn message_files<'a>(
    &'a self,
    request: &'a ChatMediaRequest,
  ) -> BoxFuture<'a, ChatAnswer<Page<MessageFile>>>;

  fn dialect_read<'a>(&'a self, text: &'a str) -> BoxFuture<'a, ChatAnswer<DialectReading>>;

  fn dialect_check<'a>(&'a self, markup: &'a str) -> BoxFuture<'a, ChatAnswer<DialectChecked>>;

  /// What the service says of the room: whether it holds it, the agent's number there, its kind, the person
  /// of a one-to-one and its title.
  fn room(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<ProviderRoom>>;

  /// A page of the rooms the service's account is in; refused where the service does not declare `roomList`.
  fn account_rooms<'a>(
    &'a self,
    query: &'a AccountRoomsQuery,
  ) -> BoxFuture<'a, ChatAnswer<Page<ProviderRoom>>>;

  /// A page of the contacts of the service's account; refused where the service does not declare
  /// `contactList`.
  fn contacts<'a>(&'a self, query: &'a ContactsQuery) -> BoxFuture<'a, ChatAnswer<Page<ChatContact>>>;

  /// A conversation started on the service, answered with its room; refused where the service does not declare
  /// the shape's capability.
  fn start_chat<'a>(&'a self, start: &'a ChatStart) -> BoxFuture<'a, ChatAnswer<ProviderRoom>>;

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

  fn send<'a>(&'a self, request: &'a ChatSendRequest) -> BoxFuture<'a, ChatAnswer<ChatSent>> {
    Box::pin(ChatProviderClient::send(self, request))
  }

  fn message_files<'a>(
    &'a self,
    request: &'a ChatMediaRequest,
  ) -> BoxFuture<'a, ChatAnswer<Page<MessageFile>>> {
    Box::pin(ChatProviderClient::message_files(self, request))
  }

  fn dialect_read<'a>(&'a self, text: &'a str) -> BoxFuture<'a, ChatAnswer<DialectReading>> {
    Box::pin(async move { ChatProviderClient::dialect_reading(self, text).await })
  }

  fn dialect_check<'a>(&'a self, markup: &'a str) -> BoxFuture<'a, ChatAnswer<DialectChecked>> {
    Box::pin(async move { ChatProviderClient::dialect_check(self, markup).await })
  }

  fn room(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<ProviderRoom>> {
    Box::pin(ChatProviderClient::room(self, chat_id))
  }

  fn account_rooms<'a>(
    &'a self,
    query: &'a AccountRoomsQuery,
  ) -> BoxFuture<'a, ChatAnswer<Page<ProviderRoom>>> {
    Box::pin(ChatProviderClient::account_rooms(self, query))
  }

  fn contacts<'a>(&'a self, query: &'a ContactsQuery) -> BoxFuture<'a, ChatAnswer<Page<ChatContact>>> {
    Box::pin(ChatProviderClient::contacts(self, query))
  }

  fn start_chat<'a>(&'a self, start: &'a ChatStart) -> BoxFuture<'a, ChatAnswer<ProviderRoom>> {
    Box::pin(ChatProviderClient::start_chat(self, start))
  }

  fn room_owns(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<bool>> {
    Box::pin(async move {
      match ChatProviderClient::room(self, chat_id).await {
        Ok(room) => Ok(room.owns),
        Err(ClientFailure::Answered { status: 404, .. }) => Ok(false),
        Err(why) => Err(why),
      }
    })
  }

  fn room_identity(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    Box::pin(async move { ChatProviderClient::room(self, chat_id).await.map(|room| room.identity) })
  }

  fn room_kind(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<RoomKind>> {
    Box::pin(async move { ChatProviderClient::room(self, chat_id).await.map(|room| room.kind) })
  }

  fn room_private_of(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    Box::pin(async move { ChatProviderClient::room(self, chat_id).await.map(|room| room.private_to) })
  }

  fn room_title(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<String>>> {
    Box::pin(async move {
      match ChatProviderClient::room(self, chat_id).await {
        Ok(room) => Ok(room.title),
        Err(ClientFailure::Answered { status: 404, .. }) => Ok(None),
        Err(why) => Err(why),
      }
    })
  }

  fn owner_room(&self) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    Box::pin(async move { self.owner().await.map(|owner| owner.room_id) })
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

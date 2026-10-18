//! A room's delegations from a shell (docs/rooms.md, 「Delegations」): what is in force there, and a grant saved
//! or taken away on the owner's own message in that room, through the web unit's own grant and revoke.

use sea_orm::DatabaseConnection;

use receiver_codes::RoomGrantRefusal;
use receiver_ports::http::PortFailure;
use receiver_ports::web::{Grant, GrantMessage, Grantee};

use crate::chat::room::room_rules;
use crate::receiver::Receiver;
use crate::verbs::VerbError;
use crate::{Fault, one};

/// The relation a room rule is: a standing rule, whose holder's file wakes the agent there unasked.
pub const STANDING_RULE: &str = "rule";

/// What is in force in a room: the room rules document, and the grants standing there on it.
pub async fn list(receiver: &Receiver, chat_id: i64) -> Result<Vec<String>, VerbError> {
  Ok(vec![room_rules(receiver, chat_id).await?])
}

/// Whom a grant is to.
pub enum Subject {
  Sender(i64),
  Members,
}

/// What is done to a room's grants: a room rule saved or deleted, or one relation granted or revoked.
pub enum Form {
  Set,
  Delete,
  Grant(String),
  Revoke(String),
}

/// One grant saved or taken away by the owner's message `message_id` in room `chat_id`.
pub struct Change {
  pub chat_id: i64,
  pub message_id: i64,
  pub form: Form,
  pub subject: Subject,
  pub words: Option<String>,
  /// Epoch milliseconds.
  pub expires_at: Option<i64>,
}

/// The change made, answered as the web unit answers it. Refused where the account model is not in force, and
/// where the message it is made on is not the owner's own in that room.
pub async fn change(
  receiver: &Receiver,
  owner_id: Option<i64>,
  change: Change,
) -> Result<Vec<String>, VerbError> {
  let web = &receiver.units().web;

  if !web.model_in_force().await.map_err(port)? {
    return Err(VerbError::Refused(RoomGrantRefusal::NoAccountModel.to_string()));
  }

  let sender = sender_of(receiver.db(), change.chat_id, change.message_id).await?;

  if owner_id.is_none() || sender != owner_id {
    return Err(VerbError::Refused(RoomGrantRefusal::NotOwnersMessage.to_string()));
  }

  let (relation, revoking) = match change.form {
    Form::Set => (STANDING_RULE.to_owned(), false),
    Form::Delete => (STANDING_RULE.to_owned(), true),
    Form::Grant(relation) => (relation, false),
    Form::Revoke(relation) => (relation, true),
  };
  let to = match change.subject {
    Subject::Sender(sender) => Grantee {
      sender: Some(sender),
      user: None,
      members: false,
    },
    Subject::Members => Grantee {
      sender: None,
      user: None,
      members: true,
    },
  };
  let message = GrantMessage {
    room: change.chat_id,
    id: change.message_id,
  };
  let grant = Grant {
    room: Some(change.chat_id),
    kind: None,
    object: None,
    relation,
    to,
    words: change.words,
    expires_at: change.expires_at,
    message,
  };
  let answer = if revoking { web.revoke(&grant).await } else { web.grant(&grant).await };

  Ok(vec![answer.map_err(port)?])
}

// The person who sent a kept message, where the room holds one.
async fn sender_of(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
) -> Result<Option<i64>, VerbError> {
  let row = one(
    db,
    r#"SELECT "sender_id" FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2"#,
    vec![chat_id.into(), message_id.into()],
  )
  .await?;

  Ok(
    row
      .map(|row| row.try_get::<Option<i64>>("", "sender_id"))
      .transpose()
      .map_err(Fault::from)?
      .flatten(),
  )
}

// A call on the web unit that did not answer.
fn port(why: PortFailure) -> VerbError {
  VerbError::Refused(why.to_string())
}

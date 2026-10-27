//! The web unit's port: the account model, a sender's user, a user, the relation checks, the grants, and the
//! words of a fixed line (`docs/boundary-api.md`, 「The web unit」). The web unit's `docs/boundary.md` states these
//! routes in the common wire format, so every body here is camelCase.

use futures_util::future::BoxFuture;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::http::{Answer, Caller, Peer};

/// A user, and whether it is the deployment's administrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
  pub user: String,
  pub administrator: bool,
}

/// A sender's relation asked in a room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomCheck {
  pub room: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sender: Option<i64>,
  pub relation: String,
}

/// A message's sender's relation asked in its room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageCheck {
  pub room: i64,
  pub message: i64,
  pub relation: String,
}

/// A user's relation asked on an object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectCheck {
  pub kind: String,
  pub object: String,
  pub relation: String,
  pub user: String,
}

/// What a check answered: whether the relation is held, the grant asked for, and the user and the sender it
/// was read for where it names them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checked {
  pub allowed: bool,
  pub grant: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub user: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sender: Option<i64>,
}

/// Whom a grant is to: a sender, a user, or every member of the room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grantee {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sender: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub user: Option<String>,
  pub members: bool,
}

/// The message a grant is made on, whose sender is the granter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantMessage {
  pub room: i64,
  pub id: i64,
}

/// A grant made or revoked: on a room, or on an object of a kind, of a relation, to its grantee, on a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub kind: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub object: Option<String>,
  pub relation: String,
  pub to: Grantee,
  /// The words the grant was made in; a revocation carries none.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub words: Option<String>,
  /// When the grant lapses, in epoch milliseconds; a revocation carries none.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub expires_at: Option<i64>,
  pub message: GrantMessage,
}

/// A fixed line asked in a language: its code and its fields beside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Render {
  pub code: String,
  #[serde(flatten)]
  pub fields: Map<String, Value>,
  pub language: String,
}

/// Every call the receiver makes on the web unit.
pub trait Web: Send + Sync {
  /// Whether the account model is in force.
  fn model_in_force(&self) -> BoxFuture<'_, Answer<bool>>;

  /// The user a sender is linked to, or none where none is linked.
  fn user_of_sender(&self, sender: i64) -> BoxFuture<'_, Answer<Option<String>>>;

  fn user<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Answer<User>>;

  /// The senders linked to a user, in the order the model gives them.
  fn senders_of_user<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Answer<Vec<i64>>>;

  /// The grants standing in a room: each its relation, the subject it is held on, when it lapses in epoch
  /// milliseconds where it does, and the words written with it where they were.
  fn room_grants(&self, chat_id: i64) -> BoxFuture<'_, Answer<Vec<GrantRow>>>;

  fn check_room<'a>(&'a self, check: &'a RoomCheck) -> BoxFuture<'a, Answer<Checked>>;

  fn check_message<'a>(&'a self, check: &'a MessageCheck) -> BoxFuture<'a, Answer<Checked>>;

  fn check_object<'a>(&'a self, check: &'a ObjectCheck) -> BoxFuture<'a, Answer<Checked>>;

  /// A grant made; the grant it stands as.
  fn grant<'a>(&'a self, grant: &'a Grant) -> BoxFuture<'a, Answer<String>>;

  /// A grant revoked; the grant it stood as.
  fn revoke<'a>(&'a self, grant: &'a Grant) -> BoxFuture<'a, Answer<String>>;

  /// A fixed line's words in a language.
  fn render<'a>(&'a self, line: &'a Render) -> BoxFuture<'a, Answer<String>>;

  /// The rooms the console has open now, from the web unit's watching route.
  fn watched_rooms(&self) -> BoxFuture<'_, Answer<Vec<i64>>>;
}

/// The web unit over HTTP.
#[derive(Debug, Clone)]
pub struct HttpWeb {
  caller: Caller,
}

impl HttpWeb {
  pub fn new(peer: &Peer) -> Self {
    Self::with_caller(Caller::new(peer))
  }

  pub fn with_caller(caller: Caller) -> Self {
    Self { caller }
  }

  async fn granted(&self, method: Method, grant: &Grant) -> Answer<String> {
    let granted: Granted = self.caller.call(method, &["grants"], grant).await?;

    Ok(granted.grant)
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Model {
  in_force: bool,
}

#[derive(Deserialize)]
struct Linked {
  user: Option<String>,
}

#[derive(Deserialize)]
struct Senders {
  senders: Vec<i64>,
}

#[derive(Deserialize)]
struct Grants {
  grants: Vec<GrantRow>,
}

/// A grant standing in a room, as the account model keeps it.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantRow {
  pub relation: String,
  pub subject_key: String,
  #[serde(default)]
  pub expires_at: Option<i64>,
  #[serde(default)]
  pub words: Option<String>,
}

#[derive(Deserialize)]
struct Granted {
  grant: String,
}

#[derive(Deserialize)]
struct Rendered {
  text: String,
}

#[derive(Deserialize)]
struct Watching {
  #[serde(default)]
  rooms: Vec<i64>,
}

impl Web for HttpWeb {
  fn model_in_force(&self) -> BoxFuture<'_, Answer<bool>> {
    Box::pin(async move {
      let model: Model = self.caller.get(&["accounts", "model"], &[]).await?;

      Ok(model.in_force)
    })
  }

  fn user_of_sender(&self, sender: i64) -> BoxFuture<'_, Answer<Option<String>>> {
    Box::pin(async move {
      let linked: Option<Linked> =
        self.caller.found(&["senders", &sender.to_string(), "user"], &[]).await?;

      Ok(linked.and_then(|linked| linked.user))
    })
  }

  fn user<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Answer<User>> {
    Box::pin(async move { self.caller.get(&["users", user], &[]).await })
  }

  fn senders_of_user<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Answer<Vec<i64>>> {
    Box::pin(async move {
      let linked: Senders = self.caller.get(&["users", user, "senders"], &[]).await?;

      Ok(linked.senders)
    })
  }

  fn room_grants(&self, chat_id: i64) -> BoxFuture<'_, Answer<Vec<GrantRow>>> {
    Box::pin(async move {
      let query = vec![("room", chat_id.to_string())];
      let standing: Grants = self.caller.get(&["grants"], &query).await?;

      Ok(standing.grants)
    })
  }

  fn check_room<'a>(&'a self, check: &'a RoomCheck) -> BoxFuture<'a, Answer<Checked>> {
    Box::pin(self.caller.post(&["checks", "room"], check))
  }

  fn check_message<'a>(&'a self, check: &'a MessageCheck) -> BoxFuture<'a, Answer<Checked>> {
    Box::pin(self.caller.post(&["checks", "message"], check))
  }

  fn check_object<'a>(&'a self, check: &'a ObjectCheck) -> BoxFuture<'a, Answer<Checked>> {
    Box::pin(self.caller.post(&["checks", "object"], check))
  }

  fn grant<'a>(&'a self, grant: &'a Grant) -> BoxFuture<'a, Answer<String>> {
    Box::pin(self.granted(Method::POST, grant))
  }

  fn revoke<'a>(&'a self, grant: &'a Grant) -> BoxFuture<'a, Answer<String>> {
    Box::pin(self.granted(Method::DELETE, grant))
  }

  fn render<'a>(&'a self, line: &'a Render) -> BoxFuture<'a, Answer<String>> {
    Box::pin(async move {
      let rendered: Rendered = self.caller.post(&["words", "render"], line).await?;

      Ok(rendered.text)
    })
  }

  fn watched_rooms(&self) -> BoxFuture<'_, Answer<Vec<i64>>> {
    Box::pin(async move {
      let watching: Watching = self.caller.get(&["watching"], &[]).await?;

      Ok(watching.rooms)
    })
  }
}

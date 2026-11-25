//! The web unit's port: the account model, a sender's user, a user, an organisation unit's people, and the words of a fixed line (`docs/boundary-api.md`, 「The web unit」). The web unit's `docs/boundary.md` states these
//! routes in the common wire format, so every body here is camelCase.

use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::http::{Answer, Caller, Peer};

/// A user, and whether it is the deployment's administrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
  pub user: String,
  pub administrator: bool,
}

/// A fixed line asked in a language: its code and its fields beside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Render {
  pub code: String,
  #[serde(flatten)]
  pub fields: Map<String, Value>,
  pub language: String,
}

/// An organisation unit as the receiver reads it: its number, its name, and the people standing in it as they stand
/// now, each by the key of their account, those whose home unit it is first and the holders of a post in it after,
/// each once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganisationUnit {
  pub id: i64,
  pub name: String,
  pub people: Vec<i64>,
}

#[derive(Deserialize)]
struct UnitView {
  id: i64,
  name: String,
  #[serde(default)]
  members: Vec<String>,
  #[serde(default)]
  concurrent: Vec<ConcurrentPost>,
}

#[derive(Deserialize)]
struct ConcurrentPost {
  user: String,
}

impl From<UnitView> for OrganisationUnit {
  fn from(view: UnitView) -> Self {
    let mut people: Vec<i64> = vec![];
    let users = view.members.iter().chain(view.concurrent.iter().map(|post| &post.user));

    // A user is an account's key as decimal text; one that reads as no key names no account the receiver holds.
    for account in users.filter_map(|user| user.trim().parse::<i64>().ok()) {
      if !people.contains(&account) {
        people.push(account);
      }
    }

    OrganisationUnit {
      id: view.id,
      name: view.name,
      people,
    }
  }
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

  /// A fixed line's words in a language.
  fn render<'a>(&'a self, line: &'a Render) -> BoxFuture<'a, Answer<String>>;

  /// The rooms the console has open now, from the web unit's watching route.
  fn watched_rooms(&self) -> BoxFuture<'_, Answer<Vec<i64>>>;

  /// The machine a user's account is bound to and whether the hub has it attached.
  fn node_of<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Answer<UserNode>>;

  /// Whether one user heads the unit another stands in, through a head grant on it or on a unit above it.
  fn heads_unit_of<'a>(&'a self, head: &'a str, user: &'a str) -> BoxFuture<'a, Answer<bool>>;

  /// An organisation unit and the people standing in it now; none where the web unit holds no such unit.
  fn organisation_unit(&self, unit: i64) -> BoxFuture<'_, Answer<Option<OrganisationUnit>>>;

  /// The company a user stands in, by its number; none where the web unit places the user in no unit, which is
  /// also all a web unit that serves no organisation answers.
  fn company_of<'a>(&'a self, _user: &'a str) -> BoxFuture<'a, Answer<Option<i64>>> {
    Box::pin(async { Ok(None) })
  }
}

#[derive(Deserialize)]
struct UnitOf {
  unit: UnitId,
}

#[derive(Deserialize)]
struct CompanyOf {
  company: UnitId,
}

#[derive(Deserialize)]
struct UnitId {
  id: i64,
}

#[derive(Serialize)]
struct UnitCheck<'a> {
  unit: i64,
  user: &'a str,
}

#[derive(Deserialize)]
struct UnitAllowed {
  allowed: bool,
}

/// The machine an account is bound to, none for an account bound to none, and whether it is attached.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct UserNode {
  #[serde(default)]
  pub agent: Option<String>,
  #[serde(default)]
  pub attached: bool,
  /// Whether the account has left.
  #[serde(default)]
  pub left: bool,
}

impl UserNode {
  /// Whether the account stands: not left, and bound to no machine or to one that is attached.
  pub fn stands(&self) -> bool {
    !self.left && (self.agent.is_none() || self.attached)
  }
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

  fn node_of<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Answer<UserNode>> {
    Box::pin(async move { self.caller.get(&["users", user, "node"], &[]).await })
  }

  fn heads_unit_of<'a>(&'a self, head: &'a str, user: &'a str) -> BoxFuture<'a, Answer<bool>> {
    Box::pin(async move {
      let Some(standing): Option<UnitOf> = self.caller.found(&["users", user, "unit"], &[]).await? else {
        return Ok(false);
      };
      let checked: UnitAllowed = self
        .caller
        .post(
          &["checks", "unit"],
          &UnitCheck {
            unit: standing.unit.id,
            user: head,
          },
        )
        .await?;

      Ok(checked.allowed)
    })
  }

  fn organisation_unit(&self, unit: i64) -> BoxFuture<'_, Answer<Option<OrganisationUnit>>> {
    Box::pin(async move {
      let found: Option<UnitView> = self.caller.found(&["units", &unit.to_string()], &[]).await?;

      Ok(found.map(OrganisationUnit::from))
    })
  }

  fn company_of<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Answer<Option<i64>>> {
    Box::pin(async move {
      let found: Option<CompanyOf> = self.caller.found(&["users", user, "unit"], &[]).await?;

      Ok(found.map(|standing| standing.company.id))
    })
  }
}

//! The port to the units declared holders of rows naming a room (`docs/boundary-api.md`, 「A declared holder」):
//! what each holds of a room, asked at the address its declaration names, behind the token the configuration
//! names for the unit standing at that address's origin.

use futures_util::future::BoxFuture;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::http::{Answer, Caller, Peer, PortFailure};

/// One thing a holder keeps of a room, by the holder's own code, and how many of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holding {
  pub code: String,
  pub count: i64,
}

#[derive(Deserialize)]
struct Held {
  holdings: Vec<Holding>,
}

/// Every call the receiver makes on a declared holder.
pub trait Holders: Send + Sync {
  /// What the holder at `address` keeps of the room, each holding named only where it keeps any.
  fn holdings<'a>(&'a self, address: &'a str, room: i64) -> BoxFuture<'a, Answer<Vec<Holding>>>;
}

/// The holders reached over HTTP, each with the token of the configured unit at its origin.
pub struct HttpHolders {
  units: Vec<Peer>,
}

impl HttpHolders {
  /// The holders as the configuration's units reach them, in the configuration's order: where several stand at
  /// one origin, the first one's token is the one sent.
  pub fn new(units: Vec<Peer>) -> Self {
    Self { units }
  }

  // The token of the first configured unit at the address's origin; none where no unit stands there.
  fn token(&self, address: &Url) -> String {
    let origin = address.origin();

    self
      .units
      .iter()
      .find(|unit| Url::parse(&unit.base).is_ok_and(|base| base.origin() == origin))
      .map(|unit| unit.token.clone())
      .unwrap_or_default()
  }
}

impl Holders for HttpHolders {
  fn holdings<'a>(&'a self, address: &'a str, room: i64) -> BoxFuture<'a, Answer<Vec<Holding>>> {
    Box::pin(async move {
      let parsed = Url::parse(address).map_err(|_| PortFailure::NotAnAddress {
        base: address.to_owned(),
      })?;
      let peer = Peer {
        base: address.to_owned(),
        token: self.token(&parsed),
      };
      let held: Held = Caller::new(&peer).get(&["rooms", &room.to_string(), "holdings"], &[]).await?;

      Ok(held.holdings)
    })
  }
}

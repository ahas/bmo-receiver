//! The agency's events as the receiver hears them (docs/boundary-api.md, 「The agency」): one stream opened on every
//! kind a hearer names, for as long as the service runs, and each event handed in turn to the hearers that name its
//! kind. A hearer is the part of the receiver its event's effect belongs to; the hearing knows none of them and is
//! handed them as the service stands. The agency waits on no reader, so a stream that drops, or could not be opened,
//! is opened again, and what the agency published meanwhile is heard by nobody.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use receiver_ports::agency::{Agency, Event};

// How long a stream that dropped or could not be opened waits before it is opened again.
const REOPEN: Duration = Duration::from_secs(5);

/// A part of the receiver that hears some of the agency's events: the kinds it hears, and what one of them does.
pub trait Hearer: Send + Sync {
  /// The kinds of event it hears, by the names the agency publishes them under.
  fn kinds(&self) -> &'static [&'static str];

  /// One event of a kind it named, heard to its end before the next event is handed over.
  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()>;
}

/// The hearers a service stands with, on the one stream they share.
#[derive(Clone)]
pub struct Hearing {
  hearers: Arc<Vec<Arc<dyn Hearer>>>,
}

impl Hearing {
  pub fn new(hearers: Vec<Arc<dyn Hearer>>) -> Self {
    Hearing {
      hearers: Arc::new(hearers),
    }
  }

  /// Every kind a hearer names, each once, in the order the hearers name them.
  pub fn kinds(&self) -> Vec<&'static str> {
    let mut kinds: Vec<&'static str> = vec![];

    for kind in self.hearers.iter().flat_map(|hearer| hearer.kinds().iter().copied()) {
      if !kinds.contains(&kind) {
        kinds.push(kind);
      }
    }

    kinds
  }

  /// One event handed to every hearer naming its kind, in the order they stand; an event no hearer names is let go.
  pub async fn hand(&self, event: &Event) {
    for hearer in self.hearers.iter().filter(|hearer| hearer.kinds().contains(&event.kind.as_str())) {
      hearer.hear(event).await;
    }
  }

  /// The stream heard for as long as the service runs, each event handed over as it comes.
  pub fn keep(self, agency: Arc<dyn Agency>) {
    tokio::spawn(async move {
      let kinds = self.kinds();

      loop {
        match agency.events(&kinds).await {
          Ok(mut stream) => {
            while let Some(read) = stream.next().await {
              match read {
                Ok(event) => self.hand(&event).await,
                Err(why) => {
                  eprintln!("bmo-receiver: the agency's event stream dropped: {why}");
                  break;
                }
              }
            }
          }
          Err(why) => eprintln!("bmo-receiver: the agency's event stream did not open: {why}"),
        }

        tokio::time::sleep(REOPEN).await;
      }
    });
  }
}

//! The agency's events as the receiver hears them (docs/boundary-api.md, 「The agency」; docs/wake.md, 「What the
//! agency's events wake」): one stream opened on every kind a hearer names, for as long as the service runs, and
//! each event handed in turn to the hearers that name its kind. A hearer is the part of the receiver its event's
//! effect belongs to; the hearing knows none of them and is handed them as the service stands. A stream that drops,
//! or could not be opened, is opened again; each time it opens, the events the agency still keeps after the last
//! one heard are handed first, where the agency's numbering still holds that one, and then everything handed what
//! the stream's opening asks of it, the reconcile of the rows handed among them.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use receiver_ports::agency::{Agency, Event};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use crate::store::settings;

// How long a stream that dropped or could not be opened waits before it is opened again.
const REOPEN: Duration = Duration::from_secs(5);

// The setting the last event heard is kept under (docs/store.md, 「The settings」).
const HEARD_KEY: &str = "agency_events_heard";

/// A part of the receiver that hears some of the agency's events: the kinds it hears, and what one of them does.
pub trait Hearer: Send + Sync {
  /// The kinds of event it hears, by the names the agency publishes them under.
  fn kinds(&self) -> &'static [&'static str];

  /// One event of a kind it named, heard to its end before the next event is handed over.
  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()>;
}

/// A part of the receiver that acts each time the stream opens, once the events it missed were handed.
pub trait Opened: Send + Sync {
  fn opened(&self) -> BoxFuture<'_, ()>;
}

/// The last event heard: the number the agency gave it and the moment it published it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
  pub id: i64,
  pub at: i64,
}

/// Where the last event heard is kept, so a stream opened again resumes after it.
pub trait Marks: Send + Sync {
  fn last(&self) -> BoxFuture<'_, Option<Mark>>;

  fn keep(&self, mark: Mark) -> BoxFuture<'_, ()>;
}

/// The last event heard kept in the receiver's settings.
pub struct HeardSetting(pub DatabaseConnection);

impl Marks for HeardSetting {
  fn last(&self) -> BoxFuture<'_, Option<Mark>> {
    Box::pin(async move {
      settings::get(&self.0, HEARD_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|kept| serde_json::from_str(&kept).ok())
    })
  }

  fn keep(&self, mark: Mark) -> BoxFuture<'_, ()> {
    Box::pin(async move {
      let written = serde_json::to_string(&mark).unwrap_or_default();

      if let Err(why) = settings::set(&self.0, HEARD_KEY, &written).await {
        eprintln!("bmo-receiver: the last of the agency's events heard was not kept: {}", why.detail);
      }
    })
  }
}

/// The hearers a service stands with, on the one stream they share.
#[derive(Clone)]
pub struct Hearing {
  hearers: Arc<Vec<Arc<dyn Hearer>>>,
  marks: Option<Arc<dyn Marks>>,
  opened: Vec<Arc<dyn Opened>>,
}

impl Hearing {
  pub fn new(hearers: Vec<Arc<dyn Hearer>>) -> Self {
    Hearing {
      hearers: Arc::new(hearers),
      marks: None,
      opened: vec![],
    }
  }

  /// The hearing keeping the last event heard where `marks` keeps it, and resuming after it.
  pub fn marking(mut self, marks: Arc<dyn Marks>) -> Self {
    self.marks = Some(marks);
    self
  }

  /// The hearing telling `opened` each time the stream opens.
  pub fn telling(mut self, opened: Arc<dyn Opened>) -> Self {
    self.opened.push(opened);
    self
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

  // One event handed, and kept as the last heard where the agency numbered it.
  async fn heard(&self, event: &Event) {
    self.hand(event).await;

    if let (Some(marks), Some(id)) = (&self.marks, event.id) {
      marks
        .keep(Mark {
          id,
          at: event.at.unwrap_or(0),
        })
        .await;
    }
  }

  // Every event the agency keeps after the number given, page after page.
  async fn kept(agency: &dyn Agency, after: Option<i64>) -> Option<Vec<Event>> {
    let mut read = vec![];
    let mut after = after;

    loop {
      let page = match agency.kept_events(after).await {
        Ok(page) => page,
        Err(why) => {
          eprintln!("bmo-receiver: the agency's kept events were not read: {why}");
          return None;
        }
      };
      let last = page.items.last().and_then(|event| event.id);

      read.extend(page.items);

      match last {
        Some(last) if page.more => after = Some(last),
        _ => return Some(read),
      }
    }
  }

  /// The events published since the last one heard that the agency still keeps, handed in order: those after it
  /// where the agency's kept events hold it under the same number and moment, every one it keeps where they do
  /// not. The number of the last event handed, which a live event at or below is not handed again; none where
  /// nothing was handed or nothing was heard before.
  pub async fn resume(&self, agency: &dyn Agency) -> Option<i64> {
    let marks = self.marks.as_ref()?;
    let mark = marks.last().await?;
    let kinds = self.kinds();
    let since = Self::kept(agency, Some(mark.id - 1)).await?;
    let heard = since
      .first()
      .is_some_and(|first| first.id == Some(mark.id) && first.at.unwrap_or(0) == mark.at);
    let missed = if heard {
      since.into_iter().skip(1).collect()
    } else {
      eprintln!(
        "bmo-receiver: the agency's kept events do not hold the last one heard ({}); every one it keeps is heard as new",
        mark.id
      );
      Self::kept(agency, None).await?
    };
    let mut last = None;

    for event in missed.iter().filter(|event| kinds.contains(&event.kind.as_str())) {
      self.heard(event).await;
      last = event.id.or(last);
    }

    last
  }

  /// The stream heard for as long as the service runs: opened, the events missed handed, the opening told, and
  /// each event handed over as it comes.
  pub fn keep(self, agency: Arc<dyn Agency>) {
    tokio::spawn(async move {
      let kinds = self.kinds();

      loop {
        match agency.events(&kinds).await {
          Ok(mut stream) => {
            let last = self.resume(agency.as_ref()).await;

            for opened in &self.opened {
              opened.opened().await;
            }

            while let Some(read) = stream.next().await {
              match read {
                Ok(event) => {
                  if last.is_none_or(|last| event.id.is_none_or(|id| id > last)) {
                    self.heard(&event).await;
                  }
                }
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

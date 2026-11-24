//! The rest's gate as the agency holds it (docs/receiver.md, 「What it carries」): the receiver closes a room's
//! turn while the deployment rests, and takes it only where the agent's one road answers through it. A rest
//! lifted past its end is reported here for the restart's clock, and a rest that silenced the deployment, as the
//! agency publishes it, is told to the owner in its fixed line. The rest itself, when a login begins it and the
//! logins it is read against, are the agency's: the receiver reads them across its port.

use futures_util::future::BoxFuture;
use receiver_codes::FixedLine;
use receiver_ports::agency::{Event, Rest, RestKind};
use serde::Deserialize;

use crate::chat::restart::notice_enabled;
use crate::hearing::Hearer;
use crate::receiver::{NotReceived, NotSaid, Receiver};
use crate::store::{Fault, settings};

// The setting the moment the rest first held a word back stands under, in epoch milliseconds, kept until the rest
// lifts and its backlog is served (docs/messages.md, 「Backlog」).
const HELD_SINCE: &str = "rest_held_since";

/// The owner's private chat's switch the rest's line and its lifting's notice stand under.
pub const NOTICE: &str = "rest";

/// A word the rest held back, sent at `sent_at` in epoch milliseconds: the earliest such moment is kept, so the
/// backlog that serves the held words as the rest lifts reaches back to it.
pub async fn note_held(receiver: &Receiver, sent_at: i64) {
  let db = receiver.db();
  let kept = settings::get(db, HELD_SINCE)
    .await
    .ok()
    .flatten()
    .and_then(|held| held.trim().parse::<i64>().ok());

  if kept.is_some_and(|kept| kept <= sent_at) {
    return;
  }

  if let Err(why) = settings::set(db, HELD_SINCE, &sent_at.to_string()).await {
    eprintln!("bmo-receiver: the moment the rest held a word was not kept: {why}");
  }
}

/// The moment the rest first held a word back, taken as it is read: none where it held none.
pub async fn take_held(receiver: &Receiver) -> Result<Option<i64>, Fault> {
  Ok(
    settings::take(receiver.db(), HELD_SINCE)
      .await?
      .and_then(|held| held.trim().parse().ok()),
  )
}

/// The moment the rest first held a word back, left in place.
pub async fn held_since(receiver: &Receiver) -> Result<Option<i64>, Fault> {
  Ok(
    settings::get(receiver.db(), HELD_SINCE)
      .await?
      .and_then(|held| held.trim().parse().ok()),
  )
}

/// The deployment's rest: whether it stands, until when, and why.
pub async fn rest(receiver: &Receiver) -> Result<Rest, NotReceived> {
  receiver.units.agency.rest().await.map_err(NotReceived::from)
}

/// Whether the deployment rests now.
pub async fn resting(receiver: &Receiver) -> Result<bool, NotReceived> {
  Ok(rest(receiver).await?.resting)
}

/// Whether an agent's one road takes a turn now, while the rest closes Claude Code's.
pub async fn road_answers(receiver: &Receiver, agent: &str) -> Result<bool, NotReceived> {
  receiver.units.agency.road_answers(agent).await.map_err(NotReceived::from)
}

/// Why the deployment rested where its rest has just passed: none while it still rests or rested for no reason
/// that ended the silence.
pub async fn clear_passed_rests(receiver: &Receiver) -> Result<Option<RestKind>, NotReceived> {
  receiver
    .units
    .agency
    .passed_rest()
    .await
    .map(|passed| passed.kind)
    .map_err(NotReceived::from)
}

/// The fixed line a rest begins with: a limit's names when it ends, an outage's says the provider stopped answering.
pub fn rest_line(kind: RestKind, until: Option<i64>) -> FixedLine {
  match kind {
    RestKind::Limit => FixedLine::RestBegins { until },
    RestKind::Outage => FixedLine::ProviderDown,
  }
}

/// The line the owner hears as the silence begins, rendered in the rooms' language where the web unit stands; none
/// where his private chat's rest switch is off.
pub async fn tell_rest(receiver: &Receiver, line: &FixedLine) -> Result<(), NotSaid> {
  if !notice_enabled(receiver.db(), receiver.owner_room(), NOTICE).await {
    return Ok(());
  }

  receiver.speech().say_line(receiver.owner_room(), line).await.map(drop)
}

/// The kind a rest is published under where it closed the last login that would answer.
pub const RESTING: &str = "rest.changed";

/// A rest that silenced the deployment: why, and until when in epoch seconds where it is known.
#[derive(Debug, Deserialize)]
pub struct Silenced {
  pub kind: RestKind,
  #[serde(default)]
  pub until: Option<i64>,
}

/// The deployment's rests as the service hears them on the agency's stream: each told to the owner in its fixed
/// line.
pub struct Rests {
  receiver: Receiver,
}

impl Rests {
  pub fn new(receiver: Receiver) -> Self {
    Rests { receiver }
  }
}

impl Hearer for Rests {
  fn kinds(&self) -> &'static [&'static str] {
    &[RESTING]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      let silenced = match serde_json::from_value::<Silenced>(event.data.clone()) {
        Ok(silenced) => silenced,
        Err(why) => {
          eprintln!("bmo-receiver: a rest that does not read: {why}");
          return;
        }
      };

      if let Err(why) = tell_rest(&self.receiver, &rest_line(silenced.kind, silenced.until)).await {
        eprintln!("bmo-receiver: the rest notice failed for the owner's chat: {why}");
      }
    })
  }
}

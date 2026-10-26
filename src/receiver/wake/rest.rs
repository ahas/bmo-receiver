//! The rest's gate as the agency holds it (docs/receiver.md, 「What it carries」): the receiver closes a room's
//! turn while the deployment rests, and takes it only where a stand-in answers on the agent's road. A rest
//! lifted past its end is reported here for the restart's clock, and a run's usage that left one of its login's
//! windows past its ceiling begins that login's rest here, and a rest that silenced the deployment, as the agency
//! publishes it, is told to the owner in its fixed line. The rest itself, and the logins it is read against,
//! are the agency's: the receiver asks and writes across its port.

use futures_util::future::BoxFuture;
use receiver_codes::FixedLine;
use receiver_policy::rest::REST;
use receiver_ports::agency::{Event, Rest, RestKind, Resting};
use serde::Deserialize;

use crate::hearing::Hearer;
use crate::receiver::{NotReceived, NotSaid, Receiver};

/// What a usage reading hands of one login: the shares of its windows past which it rests, and when each
/// window resets, in epoch seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
  pub utilization: f64,
  pub resets_at: f64,
  pub weekly_utilization: Option<f64>,
  pub weekly_resets_at: Option<f64>,
}

/// The moment a run's usage rests its login until: the five-hour window's reset where it passed its ceiling,
/// the weekly one's where that did — the policy's fallback after now where it named none — and none where
/// neither did.
pub fn rest_until_after(usage: &Usage, now: i64) -> Option<f64> {
  if usage.utilization >= REST.threshold {
    return Some(usage.resets_at);
  }

  if usage.weekly_utilization.is_some_and(|weekly| weekly >= REST.weekly_threshold) {
    return Some(usage.weekly_resets_at.unwrap_or((now + REST.fallback_seconds) as f64));
  }

  None
}

/// The moment an outage's rest lasts until, in epoch seconds: long enough to stop hammering the provider,
/// short enough that the agent is back the moment it is.
pub fn outage_rest_until(now_ms: i64) -> i64 {
  (now_ms + REST.outage_seconds * 1000).div_euclid(1000)
}

/// The moment a limit's rest lasts until: the moment the limit named, or the policy's fallback after now.
pub fn rest_until_for(resets_at: Option<i64>, now_ms: i64) -> i64 {
  resets_at.unwrap_or_else(|| (now_ms + REST.fallback_seconds * 1000).div_euclid(1000))
}

/// The deployment's rest: whether it stands, until when, and why.
pub async fn rest(receiver: &Receiver) -> Result<Rest, NotReceived> {
  receiver.units.agency.rest().await.map_err(NotReceived::from)
}

/// Whether the deployment rests now.
pub async fn resting(receiver: &Receiver) -> Result<bool, NotReceived> {
  Ok(rest(receiver).await?.resting)
}

/// Whether a stand-in answers an agent while the rest closes its road.
pub async fn stand_in(receiver: &Receiver, agent: &str) -> Result<bool, NotReceived> {
  receiver.units.agency.stand_in(agent).await.map_err(NotReceived::from)
}

/// A rest written down, answering whether it is the one that silenced the agent: a limit's rest is its login's,
/// and a login already resting is left as it stands; an outage is the service's, and a second one while the
/// first goes on writes nothing. `until` is in epoch seconds.
pub async fn write_rest(
  receiver: &Receiver,
  until: i64,
  kind: RestKind,
  account: Option<String>,
) -> Result<bool, NotReceived> {
  let written = Resting {
    until,
    kind,
    account,
  };
  let silenced = receiver.units.agency.write_rest(&written).await.map_err(NotReceived::from)?;
  let whose = written.account.as_deref().unwrap_or("the deployment");
  let silence = if silenced.silenced { "nothing answers now" } else { "something still answers" };

  println!("{whose} rests until {until}: {silence}");

  Ok(silenced.silenced)
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

/// The rest a done run's usage calls for, begun on the login the reading is of — a login already resting is
/// left as it stands — and told to the owner where it is the rest that silenced the last one that answered.
pub async fn rest_after_usage(
  receiver: &Receiver,
  usage: Option<&Usage>,
  account: &str,
  now: i64,
) -> Result<(), NotReceived> {
  let Some(until) = usage.and_then(|usage| rest_until_after(usage, now)) else {
    return Ok(());
  };
  let until = until.round() as i64;

  if write_rest(receiver, until, RestKind::Limit, Some(account.to_owned())).await?
    && let Err(why) = tell_rest(receiver, &rest_line(RestKind::Limit, Some(until))).await
  {
    eprintln!("bmo-receiver: the rest notice failed for the owner's chat: {why}");
  }

  Ok(())
}

/// The fixed line a rest begins with: a limit's names when it ends, an outage's says the provider stopped answering.
pub fn rest_line(kind: RestKind, until: Option<i64>) -> FixedLine {
  match kind {
    RestKind::Limit => FixedLine::RestBegins { until },
    RestKind::Outage => FixedLine::ProviderDown,
  }
}

/// The line the owner hears as the silence begins, rendered in the rooms' language where the web unit stands.
pub async fn tell_rest(receiver: &Receiver, line: &FixedLine) -> Result<(), NotSaid> {
  let Some(line) = receiver.speech().rendered(line).await else {
    return Ok(());
  };

  receiver.speech().say(receiver.deployment().owner_id, &line).await.map(drop)
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

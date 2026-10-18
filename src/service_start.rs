//! What the receiver does as it starts, before it answers (docs/receiver.md, 「Where it stands」): the standing
//! of one receiver on its configuration — its news heard by the event stream, its messages judged by the one
//! rule, its turns handed to the one runner whose default answerer is the awake agent its registration marks as
//! the default, and its rows handed over by the one instruction pass — and the start's work: the data root's
//! folders made, the cache warmer told of the consumer and its one target, and the agency told of the three
//! hooks the receiver answers. Every start reaches these, so what the units are told of this service is what is
//! running. A step that does not register says so and is passed over, since a service a step short still answers.

use std::collections::BTreeSet;
use std::sync::Arc;

use receiver_policy::chat_health::CHAT_HEALTH;
use receiver_ports::agency::{Agency, Agents, HttpAgency};
use receiver_ports::warmer::Target;
use sea_orm::DatabaseConnection;

use crate::chat::around::Around;
use crate::chat::conversation::{Conversations, Timings};
use crate::chat::instructions::{Pass, Rooms};
use crate::chat::restart::keep as keep_restart;
use crate::chat::runner::Runner;
use crate::companion::cut::keep_hearing;
use crate::config::Config;
use crate::data_root::DataRoot;
use crate::events::Events;
use crate::receiver::wake::Waking;
use crate::receiver::wake::agents::{Designated, default_designated};
use crate::receiver::{Receiver, Units};
use crate::tool_env::base_address;

/// The agency's hook points the receiver answers.
const HOOKS: [&str; 3] = ["room-title", "typed-line", "road-verbs"];

/// The words a room calls the chat agent by, read off its registration: its label and the words its row names,
/// each once. Where the agency does not answer, the agent's own registration name is what a room calls it by.
pub async fn call_names(agency: Arc<dyn Agency>, agent: &str) -> Vec<String> {
  let listed = agency.agents().await.unwrap_or(Agents {
    agents: vec![],
    roles: vec![],
  });

  let names: BTreeSet<String> = listed
    .agents
    .iter()
    .filter(|one| one.name == agent)
    .flat_map(|one| Designated::of(one).call_names)
    .filter(|name| !name.is_empty())
    .collect();

  let names = names.iter().map(|name| name.to_owned()).collect::<Vec<String>>();

  if names.is_empty() { vec![agent.to_owned()] } else { names }
}

/// The chat touch target, as the receiver declares it (docs/boundary-api.md, 「For the cache warmer: the chat
/// touch」, its values the receiver's own policy's).
pub fn chat_touch(agent: &str) -> Target {
  let touch = &CHAT_HEALTH.touch;

  Target {
    kind: "chat_touch".to_owned(),
    label: touch.label.clone(),
    subject: Some(agent.to_owned()),
    rank: touch.rank,
    idle_ms: touch.idle_ms,
    lifetime_ms: Some(touch.lifetime_ms),
    margin_ms: touch.margin_ms,
    ping_bound_ms: touch.ping_bound_ms,
    group: None,
    warm_by_default: Some(touch.warm_by_default),
    failure_touches: touch.failure_touches,
    honours_idle_override: touch.honours_idle_override,
    last_used_at: None,
  }
}

/// The folders a process of the data root expects before it opens a file under them: the stores that keep a
/// folder per room.
pub fn make_folders(data_root: &DataRoot) {
  for folder in [data_root.collected(), data_root.images(), data_root.outboxes()] {
    if let Err(why) = std::fs::create_dir_all(&folder) {
      eprintln!("bmo-receiver: {} could not be made: {why}", folder.display());
    }
  }
}

/// The start's work, in order, for the receiver standing on this configuration: the folders made, the warmer
/// told of the consumer and its one target, and the agency told of the three hooks it answers.
pub async fn settle(receiver: &Receiver, consumer: &str, listen: &str, token: &str) {
  make_folders(receiver.data_root());

  let base = base_address(listen);
  let units: Units = receiver.units().clone();

  let hook = format!("{base}/warmer/ping?token={token}");

  if let Err(refusal) = units.warmer.register(consumer, &hook).await {
    eprintln!("bmo-receiver: the cache warmer was not told of this consumer: {refusal}");
  }
  if let Err(refusal) = units
    .warmer
    .target(consumer, "chat-touch", &chat_touch(&receiver.deployment().agent))
    .await
  {
    eprintln!("bmo-receiver: the chat touch was not registered: {refusal}");
  }

  for point in HOOKS {
    let url = format!("{base}/agency/hooks/{point}");

    if let Err(refusal) = units.agency.register_hook(point, &url, token).await {
      eprintln!("bmo-receiver: the agency was not told of the {point} hook: {refusal}");
    }
  }
}

/// The receiver as a running service stands it: on its configuration's database, hearing its news on the
/// event stream, judging every message by the one rule, handing every woken turn to the one runner whose
/// default answerer is the awake agent its registration marks as the default, and handing every agent's rows
/// to its conversations through the one instruction pass. Nothing is started here: the clocks are the
/// service's, and a check runs one pass over the same standing.
pub struct Standing {
  pub receiver: Receiver,
  pub runner: Runner,
  pub pass: Arc<Pass>,
  pub news: Arc<Events>,
  pub conversations: Conversations,
}

impl Standing {
  /// The receiver standing on a connected database and its configuration.
  pub async fn stood(config: &Config, db: DatabaseConnection) -> Result<Self, String> {
    let agency = Arc::new(HttpAgency::new(&config.agency));
    let names = call_names(agency, &config.agent).await;

    let receiver = config.receiver(db, names);
    let news = Arc::new(Events::new());
    let mut receiver = receiver.with_news(news.clone()).with_wake(Arc::new(Waking));

    let default = default_designated(&receiver).await.map_err(|why| format!("{why}"))?;
    let runner = Runner::new(
      receiver.clone(),
      Arc::new(Around::new(receiver.clone())),
      Timings::of_policy(),
      default.name.as_str(),
    );
    let conversations = runner.default_answerer().clone();
    let rooms: Arc<dyn Rooms> = Arc::new(runner.clone());

    receiver.hold_runner(Arc::new(runner.clone()));
    receiver.clone().hold_touching(Arc::new(conversations.clone()));
    receiver.clone().hold_typing(Arc::new(conversations.clone()));
    receiver = receiver.hold_talks(Arc::new(conversations.clone()));

    let pass = Pass::new(receiver.clone(), rooms);

    receiver.hold_pass(pass.clone());

    Ok(Standing {
      receiver,
      runner,
      pass,
      news,
      conversations,
    })
  }
}

/// The clocks of a running service: the instruction pass over every agent's rows, the conversations' own
/// clock and the restart; and the companions' cut-off, heard for as long as the service runs.
pub fn keep(standing: &Standing) {
  standing.pass.clone().keep();
  standing.conversations.clone().keep_clocks();
  keep_restart(standing.receiver.clone(), standing.conversations.clone());
  keep_hearing(standing.receiver.clone(), standing.runner.clone());
}

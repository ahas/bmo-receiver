//! The health of the rooms' sessions as the conversations read it (docs/voiced-notices.md, 「System notices」;
//! docs/bmo-window.md, 「What stands in the window」): whether a room has fallen silent, what a stopped or failed
//! round is told of its room, a round that ended with nothing said at all, and the failure order a broken run of
//! the agent's own raises as a quiet instruction row — looked over on the health policy's minute, with the service's
//! own line as the last resort where no session answers the order.

use receiver_codes::FixedLine;
use receiver_policy::chat_health::{CHAT_HEALTH, Notice};
use receiver_policy::fill_marks;

use super::conversation::{BoxFuture, Failure};
use super::instructions::{Queued, queue as queue_row};
use super::notices;
use super::notices::Voiced;
use crate::receiver::wake::rest::{resting, stand_in};
use crate::receiver::{Receiver, rows};
use crate::store::Fault;
use crate::{all, now_ms, run};

// The kind a failure order of the agent's own is kept under, which the orders' clock reads back.
const ORDER_KIND: &str = "failure-order";

/// Whether a room has fallen silent: the rest closes it and nothing else would answer it.
pub async fn silenced(receiver: &Receiver, chat_id: i64) -> bool {
  if receiver.roads().is_owner_room(chat_id).await {
    return false;
  }

  let agent = receiver.deployment().agent.clone();

  resting(receiver).await.unwrap_or(false) && !stand_in(receiver, &agent).await.unwrap_or(false)
}

/// The notice a room hears of a round that stopped or failed, in the agent's own voice, with the cause the road
/// gave where it gave one.
async fn tell_health(receiver: &Receiver, chat_id: i64, said: &Notice, reason: Option<&str>) {
  let base = said.fact.clone();
  let fact = match reason.filter(|why| !why.trim().is_empty()) {
    Some(why) => format!("{base} {why}"),
    None => base,
  };
  let notice = Voiced {
    fact,
    intent: said.intent.clone(),
    follow_up: said.follow_up.clone(),
    optional: false,
    kind: Some(said.kind.clone()),
    ttl_ms: Some(said.ttl_minutes * 60_000),
    events: vec![],
    rides_ms: None,
  };

  let _ = notices::queue_notice(receiver, chat_id, &notice).await;
}

/// The order a broken run raises in its room, as a quiet instruction row the round opens on.
pub async fn raise_order(
  receiver: &Receiver,
  agent: &str,
  chat_id: i64,
  reason: Option<&str>,
) -> Result<(), Fault> {
  let where_ = rows::chat_title(receiver.db(), chat_id)
    .await
    .ok()
    .flatten()
    .unwrap_or(chat_id.to_string());
  let text = fill_marks(&CHAT_HEALTH.order.what, &[("where", &where_)]);
  let row = Queued {
    kind: Some(ORDER_KIND),
    expires_at_ms: Some(CHAT_HEALTH.order.check_seconds as i64 * 1_000),
    events: vec![],
    rides_until_ms: None,
    rides_session: false,
    sort: None,
    summary: reason,
    via: None,
    run_id: None,
  };

  queue_row(receiver.db(), agent, chat_id, &text, &row).await.map(drop)
}

/// The health of the rooms, as the conversations hold it.
#[derive(Clone)]
pub struct RoomHealth(Receiver);

impl RoomHealth {
  pub fn new(receiver: Receiver) -> Self {
    RoomHealth(receiver)
  }
}

impl crate::chat::conversation::Health for RoomHealth {
  fn silenced<'a>(&'a self, receiver: &'a Receiver, chat_id: i64) -> BoxFuture<'a, bool> {
    Box::pin(async move { silenced(receiver, chat_id).await })
  }

  fn failed<'a>(&'a self, receiver: &'a Receiver, chat_id: i64, failure: &'a Failure) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      let reason = match failure {
        Failure::Transport(why) | Failure::Failed(why) | Failure::Engine { reason: why, .. } => {
          Some(why.as_str())
        }
        _ => None,
      };
      let said = match failure {
        Failure::Stopped => &CHAT_HEALTH.stopped,
        _ => &CHAT_HEALTH.failed,
      };

      tell_health(receiver, chat_id, said, reason).await;

      if matches!(failure, Failure::Stopped) {
        return;
      }

      if let Err(why) = raise_order(receiver, &self.0.deployment().agent, chat_id, reason).await {
        eprintln!("chat {chat_id}: the failure order: {}", why.detail);
      }
    })
  }

  fn died<'a>(&'a self, receiver: &'a Receiver, chat_id: i64) -> BoxFuture<'a, ()> {
    Box::pin(async move { tell_health(receiver, chat_id, &CHAT_HEALTH.failed, None).await })
  }

  /// The rows just written are the room's next round's: the pass is handed to it now.
  fn orders(&self, receiver: &Receiver) {
    if let Some(pass) = receiver.pass() {
      pass.kick();
    }
  }
}

/// The failure orders, as the conversations' own clock looks them over.
#[derive(Clone)]
pub struct FailureOrders(Receiver);

impl FailureOrders {
  pub fn new(receiver: Receiver) -> Self {
    FailureOrders(receiver)
  }
}

impl crate::chat::conversation::Orders for FailureOrders {
  fn check_failure_orders(&self, _receiver: &Receiver, agent: &str) {
    let receiver = self.0.clone();
    let agent = agent.to_owned();

    tokio::spawn(async move {
      let due = all(
        receiver.db(),
        r#"SELECT "chat_id", "text", "created_at" FROM "instructions" WHERE "agent" = $1 AND "kind" = $2 AND "delivered_at" IS NULL AND "handed_at" IS NULL"#,
        vec![agent.clone().into(), ORDER_KIND.into()],
      )
        .await
        .ok().unwrap_or_default();
      let owner = receiver.deployment().owner_id;
      let oldest = now_ms().div_euclid(1000) - CHAT_HEALTH.order.check_seconds as i64;

      for row in due {
        let chat_id = row.try_get::<i64>("", "chat_id").ok().unwrap_or(0);
        let created = row.try_get::<i64>("", "created_at").ok().unwrap_or(0);

        if chat_id == owner || created > oldest {
          continue;
        }

        let line = receiver
          .speech()
          .rendered(&FixedLine::RunBroken {
            agent: agent.clone(),
            cause: row.try_get::<Option<String>>("", "text").ok().flatten(),
            record: None,
          })
          .await;

        let _ = receiver.speech().say(owner, &line, true).await;

        let _ = run(
          receiver.db(),
          r#"DELETE FROM "instructions" WHERE "agent" = $1 AND "chat_id" = $2 AND "kind" = $3 AND "delivered_at" IS NULL AND "handed_at" IS NULL"#,
          vec![agent.clone().into(), chat_id.into(), ORDER_KIND.into()],
        ).await;
      }
    });
  }
}

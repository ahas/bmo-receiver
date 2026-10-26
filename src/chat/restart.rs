//! The restart as the process holding the conversations meets it (docs/voiced-notices.md, 「Deploy and restart」;
//! docs/receiver.md): a rest lifted once its moment has passed, every room but the owner's read up to its latest
//! message and the owner told where it was a limit's rest; and, as the process starts, the owner's back notice
//! with the deployment's purpose, then the words a restart left unread handed to the rooms where they wake the
//! agent.

use std::collections::HashSet;
use std::time::Duration;

use receiver_codes::FixedLine;
use receiver_policy::chat_health::CHAT_HEALTH;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use receiver_policy::rest::REST;
use receiver_policy::restart::RESTART;
use sea_orm::DatabaseConnection;
use serde_json::Value;

use receiver_ports::agency::RestKind;

use super::conversation::Conversations;
use super::notices;
use super::notices::Voiced;
use crate::receiver::wake::{rest, standing};
use crate::receiver::{Inbound, Receiver, rows};
use crate::store::{Fault, settings};
use crate::{all, now_ms, one, run};

// The setting a deployment records why it restarts under.
const REASON_KEY: &str = "restart_reason";

// What one notice switch of a room says: on unless it was turned off.
pub async fn notice_enabled(db: &DatabaseConnection, chat_id: i64, kind: &str) -> bool {
  one(
    db,
    r#"SELECT "enabled" FROM "chat_notices" WHERE "chat_id" = $1 AND "kind" = $2"#,
    vec![chat_id.into(), kind.into()],
  )
  .await
  .ok()
  .flatten()
  .and_then(|row| row.try_get::<i64>("", "enabled").ok())
  .is_none_or(|enabled| enabled == 1)
}

/// The pass handed to its tick now, where one stands.
pub fn kick(receiver: &Receiver) {
  if let Some(pass) = receiver.pass() {
    pass.kick();
  }
}

/// The latest message a room holds.
async fn latest(db: &DatabaseConnection, chat_id: i64) -> Option<i64> {
  one(
    db,
    r#"SELECT "message_id" FROM "messages" WHERE "chat_id" = $1 ORDER BY "sent_at" DESC, "rowid" DESC LIMIT 1"#,
    vec![chat_id.into()],
  )
    .await
    .ok()
    .flatten()
    .and_then(|row| row.try_get::<i64>("", "message_id").ok())
}

/// A rest past its end lifted: every room but the owner's read up to its latest message, and the owner told where
/// it was a limit's rest — an outage's lifts in silence, since whether the service is back is known only when a
/// run answers.
pub async fn lift_rest_if_due(receiver: &Receiver) -> Result<(), Fault> {
  let db = receiver.db();
  let passed = rest::clear_passed_rests(receiver).await;
  let kind = match passed {
    Ok(passed) => passed,
    Err(why) => {
      eprintln!("restart: the rest could not be read: {why}");

      return Err(Fault::new(why.to_string()));
    }
  };
  let Some(kind) = kind else {
    return Ok(());
  };

  for row in all(db, r#"SELECT DISTINCT "chat_id" FROM "messages""#, vec![])
    .await
    .unwrap_or_default()
  {
    let chat_id = row.try_get::<i64>("", "chat_id").ok().unwrap_or(0);

    if receiver.roads().is_owner_room(chat_id).await {
      continue;
    }

    if let Some(latest) = latest(db, chat_id).await {
      let _ = rows::set_last_read(db, chat_id, latest).await;
    }
  }

  println!("rest lifted");

  if kind == RestKind::Outage {
    return Ok(());
  }

  let said = &CHAT_HEALTH.rest_lifted;
  let notice = Voiced {
    fact: said.fact.clone(),
    intent: said.intent.clone(),
    follow_up: said.follow_up.clone(),
    optional: false,
    kind: Some(said.kind.clone()),
    ttl_ms: Some(said.ttl_minutes * 60_000),
    events: vec![],
    rides_ms: None,
  };

  let _ = notices::queue_notice(receiver, receiver.deployment().owner_id, &notice).await;
  kick(receiver);

  Ok(())
}

/// Why the process restarted, as the deployment recorded it, read once and spent: none where no deployment
/// announced it or its record is too old to speak for this stop.
pub async fn take_reason(db: &DatabaseConnection) -> Option<Value> {
  let raw = settings::get(db, REASON_KEY).await.ok().flatten();

  let _ = run(db, r#"DELETE FROM "settings" WHERE "key" = $1"#, vec![REASON_KEY.into()]).await;

  let reason = raw.and_then(|held| serde_json::from_str::<Value>(held.as_str()).ok());

  reason.filter(|reason| {
    reason["at"].as_i64().is_some_and(|at| now_ms() - at <= RESTART.fresh_minutes * 60_000)
  })
}

/// The going-down line as it stands at a moment, under the service's own mark; none where the room turned restart
/// notices off.
pub async fn stop_line(receiver: &Receiver, at: i64) -> Option<String> {
  let owner = receiver.deployment().owner_id;

  if !notice_enabled(receiver.db(), owner, "restart").await {
    return None;
  }

  let raw = settings::get(receiver.db(), REASON_KEY).await.ok().flatten();
  let reason = raw
    .and_then(|held| serde_json::from_str::<Value>(held.as_str()).ok())
    .filter(|reason| reason["at"].as_i64().is_some_and(|held| at - held <= RESTART.fresh_minutes * 60_000));
  let line = match reason {
    None => receiver.speech().rendered(&FixedLine::GoingDown).await,
    Some(reason) => {
      let items: Vec<i64> = reason["items"]
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default();

      receiver.speech().rendered(&FixedLine::GoingDownForUpdate { items }).await
    }
  };

  Some(line)
}

/// The stopping service's line to the owner, the service's own notice since a turn woken only to say it is going
/// down costs more than the line is worth and no session outlives the stop to say it afterwards.
pub async fn say_stop(receiver: &Receiver) {
  let Some(line) = stop_line(receiver, now_ms()).await else {
    return;
  };

  if let Err(why) = receiver.speech().say(receiver.deployment().owner_id, &line, true).await {
    eprintln!("the stop notice was not said: {why}");
  }
}

/// The back notice a restart leaves the owner's session, carrying what the deployment recorded, which is spent
/// either way.
pub fn back_notice(reason: Option<&Value>) -> Voiced {
  let ttl_ms = Some(RESTART.ttl_minutes * 60_000);
  let kind = Some(RESTART.kind.clone());
  let Some(reason) = reason else {
    let said = &RESTART.unannounced;

    return Voiced {
      fact: said.fact.clone(),
      intent: said.intent.clone(),
      follow_up: Some(said.follow_up.clone()),
      optional: false,
      kind,
      ttl_ms,
      events: vec![],
      rides_ms: None,
    };
  };
  let said = &RESTART.deployed;
  let purpose = match reason["purpose"].as_str().map(str::trim).filter(|held| !held.is_empty()) {
    Some(purpose) => fill_marks(&said.purpose, &[("purpose", purpose)]),
    None => said.no_purpose.clone(),
  };
  let commit: String = reason["commit"].as_str().unwrap_or_default().chars().take(12).collect();
  let mut facts = vec![
    said.back.clone(),
    format!("{purpose}\n{}", fill_marks(&said.commit, &[("commit", &commit)])),
  ];
  let job = reason.get("job").filter(|job| job.is_object());

  if let Some(job) = job {
    let item = match (job["todoId"].as_i64(), job["title"].as_str()) {
      (None, _) => said.no_item.clone(),
      (Some(item), None) => fill_marks(&said.item, &[("item", &item.to_string())]),
      (Some(item), Some(title)) => {
        fill_marks(&said.titled_item, &[("item", &item.to_string()), ("title", title)])
      }
    };

    facts.push(fill_marks(&said.job, &[("job", &item)]));
  }

  Voiced {
    fact: facts.join("\n"),
    intent: said.intent.clone(),
    follow_up: Some(if job.is_some() { said.job_follow_up.clone() } else { said.follow_up.clone() }),
    optional: false,
    kind,
    ttl_ms,
    events: vec![],
    rides_ms: Some(RESTART.ride_minutes * 60_000),
  }
}

/// The owner's back notice queued as the process starts.
pub async fn queue_back_notice(receiver: &Receiver) {
  let db = receiver.db();
  let reason = take_reason(db).await;

  if !notice_enabled(db, receiver.deployment().owner_id, "restart").await {
    return;
  }

  let notice = back_notice(reason.as_ref());

  let _ = notices::queue_notice(receiver, receiver.deployment().owner_id, &notice).await;
  kick(receiver);
}

/// Whether the room's own road answers the agent: the room names nobody awake, or the agent among its answerers.
async fn answers(receiver: &Receiver, chat_id: i64) -> bool {
  receiver.roads().shown(chat_id).await
}

/// The numbers the room's other answerers speak under there, as the room's own road gave them.
async fn answerer_numbers(receiver: &Receiver, chat_id: i64, road: &str) -> HashSet<i64> {
  let rows = all(
    receiver.db(),
    r#"SELECT "sender_id" FROM "room_answerers" WHERE "chat_id" = $1 AND "road" = $2"#,
    vec![chat_id.into(), road.into()],
  )
  .await
  .unwrap_or_default();

  HashSet::from_iter(rows.iter().filter_map(|row| row.try_get::<i64>("", "sender_id").ok()))
}

/// The words a restart left unread, handed to the rooms where they wake the agent: in a room the agent answers and
/// that has not fallen silent, every message after its read mark that is no answerer's own — the owner's alone
/// where the room hears only him, and nobody's muted — woken on where one of them, not too old, wakes him by the
/// receiver's rule. A room with no read mark is marked read at its latest message and left.
pub async fn resume_backlog(receiver: &Receiver, conversations: &Conversations) -> Result<(), Fault> {
  let db = receiver.db();
  let owner = receiver.deployment().owner_id;
  let everyone = standing::hears_everyone(receiver).await.unwrap_or(false);
  let oldest = now_ms() - CHAT_ROUND.backlog.max_age_seconds * 1_000;
  let agent = receiver.deployment().agent.clone();
  let rooms = receiver.roads();
  let runner = receiver.runner();

  for row in all(db, r#"SELECT DISTINCT "chat_id" FROM "messages" ORDER BY "chat_id""#, vec![])
    .await
    .unwrap_or_default()
  {
    let chat_id = row.try_get::<i64>("", "chat_id").ok().unwrap_or(0);

    if !answers(receiver, chat_id).await || conversations.silenced(chat_id).await {
      continue;
    }

    let road = rooms.room_road(chat_id).await.unwrap_or_default();
    let bmo = rooms.identity_in(chat_id).await;
    let others = answerer_numbers(receiver, chat_id, &road).await;
    let Some(last_read) = rows::last_read(db, chat_id).await.ok().flatten() else {
      if let Some(latest) = latest(db, chat_id).await {
        let _ = rows::set_last_read(db, chat_id, latest).await;
      }

      continue;
    };
    let room = rooms.room_facts(chat_id).await;
    let mut unread: Vec<Inbound> = vec![];

    for message in rows::messages_after(db, chat_id, last_read, &road).await.unwrap_or_default() {
      let sender = message.sender_id;

      if sender.is_some_and(|sender| Some(sender) == bmo || others.contains(&sender)) {
        continue;
      }

      if !everyone && sender != Some(owner) {
        continue;
      }

      if let Some(sender) = sender.filter(|sender| *sender != owner)
        && standing::is_muted(receiver, chat_id, sender).await.unwrap_or(false)
      {
        continue;
      }

      unread.push(Inbound {
        via_reader: Some(false),
        ..message
      });
    }

    let mut woken = false;

    for message in &unread {
      let taken = match &runner {
        None => false,
        Some(runner) => runner.hand(&agent, &[], message, room, false, false).await,
      };

      if message.sent_at >= oldest && taken {
        woken = true;

        break;
      }
    }

    if !woken {
      continue;
    }

    println!("resuming backlog in chat {chat_id}: {} message(s)", unread.len());
    conversations.wake(unread, room).await;
  }

  Ok(())
}

/// The restart for as long as the process stands: the back notice now, the backlog once the start has settled —
/// a rest past its end lifted first — and a rest looked for on the rest policy's clock after.
pub fn keep(receiver: Receiver, conversations: Conversations) {
  tokio::spawn(async move {
    queue_back_notice(&receiver).await;

    tokio::time::sleep(Duration::from_millis(CHAT_ROUND.backlog.settle_ms)).await;

    if let Err(why) = lift_rest_if_due(&receiver).await {
      eprintln!("restart: the rest: {}", why.detail);
    }

    if let Err(why) = resume_backlog(&receiver, &conversations).await {
      eprintln!("restart: the backlog: {}", why.detail);
    }

    loop {
      tokio::time::sleep(Duration::from_secs(REST.lift_check_seconds)).await;

      if let Err(why) = lift_rest_if_due(&receiver).await {
        eprintln!("restart: the rest: {}", why.detail);
      }
    }
  });
}

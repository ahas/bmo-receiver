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
use serde_json::{Value, json};

use receiver_ports::agency::RestKind;

use super::conversation::{Conversations, unanswered};
use super::notices;
use super::notices::Voiced;
use super::relaunch::{Deployed, deployment};
use super::runner::Runner;
use crate::receiver::wake::{self, rest, standing};
use crate::receiver::{Inbound, Receiver, Room, rows};
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
    events: vec![],
    rides_ms: None,
  };

  let _ = notices::queue_notice(receiver, receiver.notices_room(), &notice).await;
  kick(receiver);

  Ok(())
}

/// Why the process restarted, read once and spent: the reason a deployment recorded, else the receiver's own
/// deployment record where it went live within the fresh window (docs/voiced-notices.md, 「Deploy and restart」);
/// none where neither speaks for this start.
pub async fn take_reason(db: &DatabaseConnection, own: Option<&Deployed>) -> Option<Value> {
  let raw = settings::get(db, REASON_KEY).await.ok().flatten();

  let _ = run(db, r#"DELETE FROM "settings" WHERE "key" = $1"#, vec![REASON_KEY.into()]).await;

  let fresh = |at: i64| now_ms() - at <= RESTART.fresh_minutes * 60_000;
  let recorded = raw
    .and_then(|held| serde_json::from_str::<Value>(held.as_str()).ok())
    .filter(|reason| reason["at"].as_i64().is_some_and(fresh));

  recorded.or_else(|| {
    own.filter(|deployed| fresh(deployed.at)).map(|deployed| {
      json!({
        "unit": deployed.unit,
        "subject": deployed.message,
        "purpose": deployed.message,
        "commit": deployed.commit,
        "at": deployed.at,
      })
    })
  })
}

/// The receiver's own deployment, as the record under the deploy root its release stands in tells it: the first
/// folder above the running binary that holds a deployment record.
pub fn own_deployment() -> Option<Deployed> {
  let binary = std::env::current_exe().ok()?;

  binary.ancestors().skip(1).find_map(|folder| {
    let unit = folder.file_name()?.to_string_lossy().trim_start_matches("bmo-").to_owned();

    folder.join("deployed.json").is_file().then(|| deployment(folder, &unit)).flatten()
  })
}

/// The going-down line as it stands at a moment, under the service's own mark; none where the room turned restart
/// notices off.
pub async fn stop_line(receiver: &Receiver, at: i64) -> Option<String> {
  let owner = receiver.owner_room();

  if !notice_enabled(receiver.db(), owner, "restart").await {
    return None;
  }

  let raw = settings::get(receiver.db(), REASON_KEY).await.ok().flatten();
  let reason = raw
    .and_then(|held| serde_json::from_str::<Value>(held.as_str()).ok())
    .filter(|reason| reason["at"].as_i64().is_some_and(|held| at - held <= RESTART.fresh_minutes * 60_000));
  match reason {
    None => receiver.speech().rendered(&FixedLine::GoingDown).await,
    Some(reason) => {
      let items: Vec<i64> = reason["items"]
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default();

      receiver.speech().rendered(&FixedLine::GoingDownForUpdate { items }).await
    }
  }
}

/// The stopping service's line to the owner, the service's own notice since a turn woken only to say it is going
/// down costs more than the line is worth and no session outlives the stop to say it afterwards.
pub async fn say_stop(receiver: &Receiver) {
  let Some(line) = stop_line(receiver, now_ms()).await else {
    return;
  };

  if let Err(why) = receiver.speech().say(receiver.owner_room(), &line).await {
    eprintln!("the stop notice was not said: {why}");
  }
}

/// The back notice a restart leaves the owner's session, carrying what the deployment recorded, which is spent
/// either way.
pub fn back_notice(reason: Option<&Value>) -> Voiced {
  let kind = Some(RESTART.kind.clone());
  let Some(reason) = reason else {
    let said = &RESTART.unannounced;

    return Voiced {
      fact: said.fact.clone(),
      intent: said.intent.clone(),
      follow_up: Some(said.follow_up.clone()),
      optional: false,
      kind,
      events: vec![],
      rides_ms: None,
    };
  };
  let said = &RESTART.deployed;
  let unit = reason["unit"].as_str().map(str::trim).filter(|held| !held.is_empty());
  let subject = reason["subject"]
    .as_str()
    .or_else(|| reason["purpose"].as_str())
    .map(str::trim)
    .filter(|held| !held.is_empty());
  let purpose = match reason["purpose"].as_str().map(str::trim).filter(|held| !held.is_empty()) {
    Some(purpose) => fill_marks(&said.purpose, &[("purpose", purpose)]),
    None => said.no_purpose.clone(),
  };
  let commit: String = reason["commit"].as_str().unwrap_or_default().chars().take(12).collect();
  let mut facts = vec![said.back.clone()];

  if let Some(unit) = unit {
    facts.push(fill_marks(&said.unit, &[("unit", unit)]));
  }

  facts.push(format!("{purpose}\n{}", fill_marks(&said.commit, &[("commit", &commit)])));

  if let Some(subject) = subject {
    facts.push(fill_marks(&said.subject, &[("subject", subject)]));
  }

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

  let cut = match reason["cut"].as_str() {
    Some("turn") => &said.cut_turn,
    Some("job") => &said.cut_job,
    _ if reason["turnCut"].as_bool() == Some(true) => &said.cut_turn,
    _ if reason["jobCut"].as_bool() == Some(true) => &said.cut_job,
    _ => &said.cut_none,
  };

  facts.push(cut.clone());

  Voiced {
    fact: facts.join("\n"),
    intent: said.intent.clone(),
    follow_up: Some(if job.is_some() { said.job_follow_up.clone() } else { said.follow_up.clone() }),
    optional: false,
    kind,
    events: vec![],
    rides_ms: Some(RESTART.ride_minutes * 60_000),
  }
}

/// The owner's back notice queued as the process starts.
pub async fn queue_back_notice(receiver: &Receiver) {
  let db = receiver.db();
  let own = own_deployment();
  let mut reason = take_reason(db, own.as_ref()).await;

  if let Some(reason) = reason.as_mut() {
    let cut = one(
      db,
      r#"SELECT COUNT(*) AS "cut" FROM "instructions"
         WHERE "delivered_at" IS NULL AND ("handed_at" IS NOT NULL OR "restarts" > 0)"#,
      vec![],
    )
    .await
    .ok()
    .flatten()
    .and_then(|row| row.try_get::<i64>("", "cut").ok())
    .unwrap_or(0);

    if cut > 0 && reason["cut"].is_null() {
      reason["cut"] = json!("turn");
    }
  }

  if !notice_enabled(db, receiver.owner_room(), "restart").await {
    return;
  }

  let notice = back_notice(reason.as_ref());

  let _ = notices::queue_notice(receiver, receiver.owner_room(), &notice).await;
  kick(receiver);
}

/// The numbers the room's other answerers speak under there, as the room's own road gave them.
async fn answerer_numbers(receiver: &Receiver, chat_id: i64, road: &str) -> HashSet<i64> {
  let rows = all(
    receiver.db(),
    r#"SELECT "sender_id" FROM "room_supervisors" WHERE "chat_id" = $1 AND "road" = $2"#,
    vec![chat_id.into(), road.into()],
  )
  .await
  .unwrap_or_default();

  HashSet::from_iter(rows.iter().filter_map(|row| row.try_get::<i64>("", "sender_id").ok()))
}

/// The lines an agent's backlog turn takes (docs/messages.md, 「Backlog」): from the first unread line sent since
/// `since` that wakes it by the one rule, that line and every unread line after it, as a live round takes the words
/// that came after the call that woke it; none where no such line wakes it.
async fn from_first_waking(
  receiver: &Receiver,
  agent: &str,
  unread: &[Inbound],
  room: Room,
  since: i64,
) -> Vec<Inbound> {
  for (at, message) in unread.iter().enumerate() {
    if message.sent_at >= since && wake::wakes(receiver, agent, message, room).await.unwrap_or(false) {
      return unread[at..].to_vec();
    }
  }

  vec![]
}

/// The words a restart left unread in every room the receiver listens to, one room at a time in room order, each
/// agent of the room handed the lines from the first that wakes it (docs/messages.md, 「Backlog」). The read mark
/// the conversation writes after the turn keeps a line from being served twice; a room with nothing waiting is not
/// served and says nothing.
pub async fn resume_backlog(receiver: &Receiver, runner: &Runner) -> Result<(), Fault> {
  let db = receiver.db();
  let owner = receiver.deployment().owner_id;
  let rooms = receiver.roads();
  let since = now_ms() - CHAT_ROUND.backlog.within_minutes * 60_000;
  let mut open: Vec<i64> = all(db, r#"SELECT DISTINCT "chat_id" FROM "messages""#, vec![])
    .await?
    .iter()
    .filter_map(|row| row.try_get::<i64>("", "chat_id").ok())
    .collect();

  open.sort_unstable();
  open.dedup();

  for chat_id in open {
    // A room the receiver does not listen to starts no session, however much it holds unread (docs/wake.md,
    // 「The room's queue and the round」).
    if !receiver.listens(chat_id).await.unwrap_or(false) {
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

    if unread.is_empty() {
      continue;
    }

    let agents = match wake::agents::room_designated(receiver, chat_id, room).await {
      Ok((agents, _)) => agents.into_iter().map(|agent| agent.name).collect::<Vec<_>>(),
      Err(why) => {
        eprintln!("restart: chat {chat_id}'s answerers were not read, so its backlog waits: {why}");
        continue;
      }
    };
    let last = unread.last().map(|message| message.message_id).unwrap_or_default();
    let mut served: Vec<Conversations> = vec![];
    let mut resting: Vec<&str> = vec![];

    for agent in &agents {
      let words = from_first_waking(receiver, agent, &unread, room, since).await;

      if words.is_empty() {
        continue;
      }

      let conversations = runner.of(agent);

      if conversations.silenced(chat_id).await {
        resting.push(agent);
        continue;
      }

      println!("resuming backlog in chat {chat_id} for {agent}: {} message(s)", words.len());
      conversations.wake(words, room).await;
      served.push(conversations);
    }

    // Words no answerer took because the rest closes its road are said once, as the wake says them.
    if served.is_empty() && !resting.is_empty() {
      let held: Vec<i64> = unread.iter().map(|message| message.message_id).collect();

      unanswered(chat_id, &held, &format!("the road of {} is closed by the rest", resting.join(", ")));
    }

    // One room at a time: the next room waits until this room's turns have gone out and its read mark stands at
    // the last line handed, which is what keeps a served line from being served again, or until the room holds
    // nothing more to answer for any agent served, as a turn that failed or was stopped leaves it. However long
    // the turns take, the next room waits for them.
    while !served.is_empty()
      && !rows::is_read(db, chat_id, last).await.unwrap_or(false)
      && !served.iter().all(|conversations| conversations.settled(chat_id))
    {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  }

  Ok(())
}

/// The restart for as long as the process stands: the back notice now, the backlog once the start has settled —
/// a rest past its end lifted first — and a rest looked for on the rest policy's clock after.
pub fn keep(receiver: Receiver, runner: Runner) {
  tokio::spawn(async move {
    queue_back_notice(&receiver).await;

    tokio::time::sleep(Duration::from_millis(CHAT_ROUND.backlog.settle_ms)).await;

    if let Err(why) = lift_rest_if_due(&receiver).await {
      eprintln!("restart: the rest: {}", why.detail);
    }

    if let Err(why) = resume_backlog(&receiver, &runner).await {
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

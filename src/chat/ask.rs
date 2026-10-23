//! One turn of a conversational agent's session (docs/bmo-window.md, 「The window」; the agency's `docs/api.md`,
//! 「Sessions」): the receiver composes it and hands it across the boundary, the agency carries it into the
//! agent's program and answers with what the session said. The scope is the receiver's declaration — one session
//! across the rooms for an agent whose registration keeps it so, a room's session otherwise — and a session the
//! road says may not be resumed is ended here and started again, the note it left riding at the head of the
//! fresh one's first turn. What the turn's run leaves in the agency's log is drawn into the room's picture of
//! the turn as it goes.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::Value;
use tokio::time::MissedTickBehavior;

use receiver_codes::SessionReason;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_ports::agency::{Agency, Delivery, Event, Said, Scope, Session, Turn, TurnRoom};
use receiver_ports::http::PortFailure;

use super::cache::{
  Leaving, Route, carried_note, pending_note, remember, reset_reason, without_archive_replay,
};
use super::context::{RoomSend, State, loaded_in, remember_held, sent_prompt, states};
use super::conversation::{
  Aside, Asking, BoxFuture, Carrying, Failure, Follower, Held, Reply, unified_session,
};
use super::progress::{RunProgress, picture_of};
use super::room::{RoomContext, Trees, base_persona, room_context};
use crate::now_ms;
use crate::receiver::Receiver;
use crate::room_record::RoomForget;

// The least a room's first launch since this process started is spaced from another room's: several sessions
// starting from cold at once is the shape of abuse.
const LAUNCH_GAP: Duration = Duration::from_secs(10);

static PACING: LazyLock<tokio::sync::Mutex<(HashSet<i64>, i64)>> =
  LazyLock::new(|| tokio::sync::Mutex::new((HashSet::new(), 0)));

static OPENED: Mutex<u64> = Mutex::new(0);

/// A fresh name for one run of a room's conversation, since two runs of a room can open in the same moment.
pub fn chat_run_name(chat_id: i64) -> String {
  let mut opened = OPENED.lock().unwrap_or_else(PoisonError::into_inner);

  *opened += 1;

  format!("chat:{chat_id}:{}:{}", now_ms(), *opened)
}

// A room whose registration is removed is a room this process has not launched for, should it come again.
fn forget_room(_: &Receiver, room: i64) -> BoxFuture<'_, ()> {
  Box::pin(async move {
    let _ = PACING.lock().await.0.remove(&room);
  })
}

crate::room_forget!(RoomForget {
  what: "the first-launch pacing of the room",
  forget: forget_room,
});

/// The first run of each room since this process started, spaced from the first run of any other room.
async fn pace_first_launch(chat_id: i64) {
  let mut pacing = PACING.lock().await;

  if !pacing.0.insert(chat_id) {
    return;
  }

  let wait = pacing.1 + LAUNCH_GAP.as_millis() as i64 - now_ms();

  if wait > 0 {
    tokio::time::sleep(Duration::from_millis(wait as u64)).await;
  }

  pacing.1 = now_ms();
}

/// What a turn was handed, by where it came from, what it is and how it went in: the stretch of the room where it
/// went, then each document the turn was built with, held where the session already had it.
pub fn handed_of(
  chat_id: i64,
  send: Option<RoomSend>,
  built: &[(String, String)],
  held: &HashMap<String, State>,
) -> Vec<(String, String, Option<&'static str>)> {
  let room = match send {
    Some(send @ (RoomSend::Full | RoomSend::Mark)) => {
      Some(("room".to_owned(), format!("chat {chat_id}"), (send == RoomSend::Mark).then_some("held")))
    }
    _ => None,
  };
  let documents = built.iter().map(|(source, text)| {
    let state = held.get(text).map(|state| match state {
      State::Held => "held",
      State::Attached => "attached",
    });

    (source.clone(), text.clone(), state)
  });

  room.into_iter().chain(documents).collect()
}

/// Why the agency's answer to a session operation ended the turn.
fn fault_of(why: PortFailure) -> Failure {
  Failure::Failed(why.to_string())
}

/// The session scope an agent's turns go on: one across its rooms, or the room's own.
pub fn scope_of(unified: bool, chat_id: i64) -> Scope {
  if unified { Scope::Agent } else { Scope::Room { room: chat_id } }
}

/// The turn the agency carries: its prompt and the context the receiver composed around it.
fn turn_of(
  asking: &Asking,
  session: &Session,
  room: &RoomContext,
  prompt: &str,
  persona: Option<String>,
) -> Turn {
  Turn {
    session: session.id.to_owned(),
    prompt: prompt.to_owned(),
    room: Some(TurnRoom {
      chat_id: asking.chat_id,
      text: room.text.to_owned(),
      mark: room.mark.to_owned(),
    }),
    via: asking.via.clone(),
    base_persona: persona,
    privileged: Some(asking.privileged),
    answers: asking.answers.to_vec(),
    began: Some(asking.began),
  }
}

/// The carrying of a composed turn into the agent's session, as the conversations hold it: the receiver it
/// carries for, and how often a working turn is looked at for words that came for its room.
#[derive(Clone)]
pub struct Carry {
  receiver: Receiver,
  look: Duration,
}

impl Carry {
  /// The carrying looking at a working turn as often as the round policy says.
  pub fn new(receiver: Receiver) -> Self {
    Carry::looking_every(receiver, Duration::from_millis(CHAT_ROUND.pushing.look_ms))
  }

  /// The carrying looking at a working turn every `look`.
  pub fn looking_every(receiver: Receiver, look: Duration) -> Self {
    Carry { receiver, look }
  }

  pub async fn carries(&self, asking: &Asking, held: &Held, follower: &Follower) -> Result<Reply, Failure> {
    let receiver = &self.receiver;
    let agency = receiver.units().agency.clone();
    let db = receiver.db();
    let agent = held.agent.as_str();
    let chat_id = asking.chat_id;

    pace_first_launch(chat_id).await;

    let unified = unified_session(receiver, agent).await;
    let scope = scope_of(unified, chat_id);
    let started = agency.start(agent, scope).await.map_err(fault_of)?;
    let started_id = started.id.to_owned();

    let room = room_context(receiver, receiver.owner_id(), chat_id, asking.via.as_deref())
      .await
      .map_err(|why| Failure::Failed(why.detail))?;

    let (sent, send) = sent_prompt(receiver, Some(started_id.as_str()), Some(&room), true, &asking.prompt)
      .await
      .map_err(|why| Failure::Failed(why.detail))?;
    let loaded = loaded_in(receiver, Some(started_id.as_str()))
      .await
      .map_err(|why| Failure::Failed(why.detail))?;
    let marked = states(&asking.prompt, &loaded);
    // A session the agency has just started is not opened until its first turn, so a state it answers
    // "not opened" for is the fresh session's, with no provider or model to resume yet.
    let standing = agency.state(&started_id).await.unwrap_or_default();

    // The one session an agent keeps across its rooms, where the road says it may not be resumed, is let go
    // here and started again, the agent's pending note — the one note it left — riding at the head. A room's
    // own session is the agency's to resume.
    let asked = Route {
      provider: standing.provider.as_deref().unwrap_or_default().to_owned(),
      model: standing.model.as_deref().unwrap_or_default().to_owned(),
      effort: String::new(),
      account: None,
      session: Some(started_id.clone()),
      transcript: None,
    };
    let reason = if unified {
      reset_reason(db, agent, &asked, &sent)
        .await
        .map_err(|why| Failure::Failed(why.detail))?
    } else {
      None
    };

    let (session, prompt, replaced): (Session, String, Vec<SessionReason>) = match reason {
      None => (started, sent, vec![]),
      Some(reason) => {
        agency.end(&started_id).await.map_err(fault_of)?;

        let note = pending_note(db, agent).await.ok().flatten();
        let again = agency.start(agent, scope).await.map_err(fault_of)?;

        (
          again,
          format!("{}\n\n{}", carried_note(note.as_deref()), without_archive_replay(&sent)),
          vec![reason],
        )
      }
    };

    let base = base_persona(&Trees::of(receiver));
    let turn = turn_of(asking, &session, &room, &prompt, base);
    let kinds: Vec<&'static str> = vec!["turn.created", "turn.ended"];
    let mut stream = agency.events(&kinds).await.map_err(fault_of)?;

    // The turn is sent apart, so the room's picture of it can be drawn from the run's log while it works.
    let port = agency.clone();
    let sending = tokio::spawn(async move { port.turn(&turn).await });

    let handed = handed_of(chat_id, send, &asking.context, &marked);
    let mut picture = RunProgress::starting(&handed);
    let mut run: Option<String> = None;
    let mut turning: Option<i64> = None;
    let mut after: i64 = 0;
    let mut ended: Option<Value> = None;
    let mut look = tokio::time::interval(self.look.max(Duration::from_millis(1)));
    let mut quiet_until: Option<tokio::time::Instant> = None;

    look.set_missed_tick_behavior(MissedTickBehavior::Delay);

    while ended.is_none() {
      let read = tokio::select! {
        read = stream.next() => read,
        _ = look.tick() => {
          if let Some(turning) = turning
            && quiet_until.is_none_or(|until| tokio::time::Instant::now() >= until)
          {
            quiet_until = pushed(agency.as_ref(), &session.id, turning, follower).await;
          }

          continue;
        }
      };

      match read {
        None => break,
        Some(Ok(Event { kind, data })) => {
          let body = &data;
          let mine = body.get("session").and_then(Value::as_str).is_some_and(|one| one == session.id);

          if mine && kind == "turn.created" {
            run = body.get("run").and_then(Value::as_str).map(str::to_owned);
            turning = body.get("turnId").and_then(Value::as_i64);
          } else if mine && kind == "turn.ended" {
            ended = Some(body.clone());
          }
        }
        Some(Err(why)) => return Err(fault_of(why)),
      }

      if let Some(run) = &run {
        let since = if after == 0 { None } else { Some(after) };

        for line in agency.run_log(run, since).await.map_err(fault_of)? {
          let text = line
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| serde_json::to_string(&line).unwrap_or_default());

          if let Some(read) = picture.read(&text) {
            picture = read;
          }

          after = line.get("id").and_then(Value::as_i64).unwrap_or(after);
        }

        follower.progressed(picture_of(&picture));
      }
    }

    drop(stream);

    let delivered =
      sending.await.map_err(|_| Failure::Failed("the turn's carrying went away".to_owned()))?;
    let delivered = delivered.map_err(fault_of)?;

    // A session's first turn opens it as the agency carries the turn; a state it has not answered for
    // yet is read as the fresh session's, with nothing of a request behind it.
    let standing = agency.state(&session.id).await.unwrap_or_default();
    let ended: Value = ended.clone().unwrap_or(Value::Null);
    let answer = ended.get("answer").and_then(Value::as_str).unwrap_or_default();
    let outcome = ended.get("outcome").and_then(Value::as_str).unwrap_or_default();

    if delivered == Delivery::Delivered {
      let _ =
        remember_held(receiver, Some(started_id.as_str()), &session.id, Some(&room), true, &asking.prompt)
          .await;

      let _ = remember(
        db,
        agent,
        &Leaving {
          route: Route {
            provider: standing.provider.as_deref().unwrap_or_default().to_owned(),
            model: standing.model.as_deref().unwrap_or_default().to_owned(),
            effort: String::new(),
            account: None,
            session: Some(session.id.to_owned()),
            transcript: None,
          },
          sent: prompt.to_owned(),
          resumed: Some(started_id),
          cache_reads: standing.context.get("cacheReadTokens").and_then(Value::as_i64),
          input: standing.context.get("inputTokens").and_then(Value::as_i64),
          text: answer.to_owned(),
        },
      )
      .await;
    }

    let reason = ended.get("reason").and_then(Value::as_str).unwrap_or_default().to_owned();

    match outcome {
      "stopped" => Err(Failure::Stopped),
      // A turn its engine ended with an error says so, with the error's kind where the engine named one.
      "failed" => Err(match ended.get("engineError").filter(|engine| engine.is_object()) {
        Some(engine) => Failure::Engine {
          reason,
          kind: engine.get("kind").and_then(Value::as_str).map(str::to_owned),
        },
        None => Failure::Failed(reason),
      }),
      _ => Ok(Reply {
        text: answer.to_owned(),
        mode: standing.provider,
        account: None,
        session: Some(session.id),
        context_tokens: standing.context.get("carried").and_then(Value::as_i64),
        context_window: standing.context.get("window").and_then(Value::as_i64),
        replaced: replaced.clone(),
        run: run.clone(),
      }),
    }
  }
}

/// The words that came for the round's room while its turn works, offered to the turn and settled on what the
/// agency said became of them; answered with the moment before which nothing more is offered, where a refusal
/// or a push that failed asks for that gap.
pub async fn pushed(
  agency: &dyn Agency,
  session: &str,
  turn: i64,
  aside: &dyn Aside,
) -> Option<tokio::time::Instant> {
  let words = aside.offer().await?;
  let said = match agency.push(session, turn, &words).await {
    Ok(said) => said,
    Err(why) => {
      eprintln!("session {session}: words for the working turn did not go in: {why}");
      Said::Refused
    }
  };

  aside.settle(said.into()).await;

  (said == Said::Refused)
    .then(|| tokio::time::Instant::now() + Duration::from_millis(CHAT_ROUND.pushing.refused_gap_ms))
}

impl Carrying for Carry {
  fn carry<'a>(
    &'a self,
    asking: &'a Asking,
    held: &'a Held,
    follower: &'a Follower,
  ) -> BoxFuture<'a, Result<Reply, Failure>> {
    Box::pin(self.carries(asking, held, follower))
  }
}

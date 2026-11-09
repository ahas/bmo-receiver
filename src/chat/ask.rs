//! One turn of a conversational agent's session (docs/bmo-window.md, 「The window」; the agency's `docs/api.md`,
//! 「Sessions」): the receiver composes it and hands it across the boundary, the agency carries it into the
//! agent's program and answers with what the session said. The scope is the receiver's declaration — one session
//! across the rooms for an agent whose registration keeps it so, a room's session otherwise — and a session the
//! road says may not be resumed is ended here and started again, the note it left riding at the head of the
//! fresh one's first turn. What the turn's run leaves in the agency's log is drawn into the room's picture of
//! the turn as it goes.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::Value;
use tokio::time::MissedTickBehavior;

use receiver_codes::SessionReason;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::handover::HANDOVER;
use receiver_ports::agency::{
  Agency, DOUBLE_SOURCE, Delivery, DoubleStart, DoubleTurn, Event, Said, Scope, Session, Turn, TurnRoom,
};
use receiver_ports::http::PortFailure;

use super::cache::{Leaving, Route, remember, reset_reason, spend_force, transcript};
use super::context::{RoomSend, State, loaded_in, remember_held, sent_prompt, states};
use super::conversation::{
  Aside, Asking, BoxFuture, Carrying, Doubled, Failure, Follower, Held, Reply, unified_session,
};
use super::generation::{self, Standing};
use super::handoff::{self, Note, carried_note};
use super::progress::{RunProgress, picture_of};
use super::room::{RoomContext, room_context};
use super::sentinels::is_handoff;
use crate::now_ms;
use crate::receiver::Receiver;
use crate::room_record::RoomForget;
use crate::rounds;
use crate::run_lines::thinking_of;

// The least a room's first launch since the receiver started is spaced from another room's: several sessions
// starting from cold at once is the shape of abuse.
const LAUNCH_GAP: Duration = Duration::from_secs(10);

static OPENED: Mutex<u64> = Mutex::new(0);

/// A fresh name for one run of a room's conversation, since two runs of a room can open in the same moment.
pub fn chat_run_name(chat_id: i64) -> String {
  let mut opened = OPENED.lock().unwrap_or_else(PoisonError::into_inner);

  *opened += 1;

  format!("chat:{chat_id}:{}:{}", now_ms(), *opened)
}

// A room whose registration is removed is a room the receiver has not launched for, should it come again.
fn forget_room(receiver: &Receiver, room: i64) -> BoxFuture<'_, ()> {
  Box::pin(async move {
    let _ = receiver.launches().lock().await.rooms.remove(&room);
  })
}

crate::room_forget!(RoomForget {
  what: "the first-launch pacing of the room",
  forget: forget_room,
});

/// The first run of each room since the receiver started, spaced from the first run of any other room: each first
/// launch takes the next free moment on the receiver's one schedule and waits for it alone, so a room already
/// launched, and every other room's launch, never waits behind it (docs/bmo-window.md, 「Quiet instruction」).
async fn pace_first_launch(receiver: &Receiver, chat_id: i64) {
  let wait = {
    let mut launches = receiver.launches().lock().await;

    if !launches.rooms.insert(chat_id) {
      return;
    }

    let now = now_ms();
    let slot = (launches.last + LAUNCH_GAP.as_millis() as i64).max(now);

    launches.last = slot;
    slot - now
  };

  if wait > 0 {
    tokio::time::sleep(Duration::from_millis(wait as u64)).await;
  }
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

/// The situation notes an agent owes written by its session before that session is asked for its handoff
/// (docs/unified-bmo.md, 「Replacing the session」), through the agent's conversations where they stand; a note
/// that is not written stays owed.
async fn owed_notes(receiver: &Receiver, agent: &str, session: &str) {
  let Some(runner) = receiver.runner() else {
    return;
  };

  runner.owed_notes(agent, session).await;
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
fn turn_of(asking: &Asking, session: &Session, room: &RoomContext, prompt: &str) -> Turn {
  Turn {
    session: session.id.to_owned(),
    prompt: prompt.to_owned(),
    room: Some(TurnRoom {
      chat_id: asking.chat_id,
      text: room.text.to_owned(),
      mark: room.mark.to_owned(),
    }),
    via: asking.via.clone(),
    privileged: Some(asking.privileged),
    answers: asking.answers.to_vec(),
    began: Some(asking.began),
    round: rounds::current(),
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

  /// The provider the agency's state names for the session a room's turn goes on, read before the turn: a
  /// session's state answers it from its start. None where the agency does not say.
  pub async fn standing(&self, agent: &str, chat_id: i64) -> Option<String> {
    let agency = self.receiver.units().agency.clone();
    let scope = scope_of(unified_session(&self.receiver, agent).await, chat_id);
    let started = agency.start(agent, scope).await.ok()?;

    agency.state(&started.id).await.unwrap_or_default().provider
  }

  /// The agent's one session replaced: asked one more turn for its handoff file, ended with what it held, the
  /// owner's word to start afresh spent, and started again; the new session and the note its first turn opens
  /// with: the handoff, else the receiver's transcript of the old session, else the word that nothing was
  /// recovered, of which the owner is told.
  async fn replace(
    &self,
    agent: &str,
    old: &str,
    scope: Scope,
    chat_id: i64,
    reason: SessionReason,
  ) -> Result<(Session, String), Failure> {
    let receiver = &self.receiver;
    let agency = receiver.units().agency.clone();
    let db = receiver.db();

    // The session about to end writes the situation notes it owes before it is asked for its handoff.
    owed_notes(receiver, agent, old).await;

    let handed = handoff::ask(receiver, agent, old, chat_id).await;

    agency.end(old).await.map_err(fault_of)?;

    // The ended session's loads, its room's block and its cache record go with it; the one started next holds
    // nothing yet.
    generation::forget(receiver, agent, old).await;

    if let Err(why) = spend_force(db, agent).await {
      eprintln!("{agent}: the word to start afresh was not spent: {}", why.detail);
    }

    let note = match handed {
      Some(handed) => Note::Handed(handed),
      None => match transcript(db, agent).await.ok().flatten() {
        Some(kept) => Note::Carried(kept),
        None => Note::Lost,
      },
    };

    if note == Note::Lost {
      handoff::tell_lost(receiver, agent, reason).await;
    }

    let again = agency.start(agent, scope).await.map_err(fault_of)?;

    if let Some(times) = handoff::in_a_row(receiver, agent, &format!("replacement-{reason}"), true).await {
      eprintln!(
        "{agent}: session replaced again for the same reason ({reason}), {times} in a row, each within {} minutes of \
         the last",
        HANDOVER.again_minutes
      );
    }

    println!(
      "{agent}: session {old} replaced ({reason}), the new one opening on {}",
      match &note {
        Note::Handed(handed) => handed.path.display().to_string(),
        Note::Carried(_) => "the transcript".to_owned(),
        Note::Lost => "nothing recovered".to_owned(),
      }
    );

    Ok((again, carried_note(&note)))
  }

  /// The agent's standing session asked for its handoff ahead of a fresh session the agency is about to open
  /// (docs/api.md, 「Handoffs」), exactly as a replacement asks it, and the file kept for the next fresh session's
  /// first turn: the file where it was written, none where the agent has no session to ask or it wrote nothing.
  /// The agency lets its record of the session go before it asks, so the state it answers names no generation and
  /// is no reason not to ask; only the asking itself tells whether a session stood.
  pub async fn hand_off(
    &self,
    agent: &str,
    room: Option<i64>,
  ) -> Result<Option<handoff::Handoff>, Failure> {
    let receiver = &self.receiver;
    let agency = receiver.units().agency.clone();
    let unified = unified_session(receiver, agent).await;
    let chat_id = room.unwrap_or_else(|| receiver.owner_room());
    let scope = scope_of(unified, chat_id);
    let standing = match agency.start(agent, scope).await {
      Ok(standing) => standing,
      Err(why) if matches!(why.status(), Some(404 | 422)) => {
        eprintln!("{agent}: no session was started to ask for a handoff: {why}");

        return Ok(None);
      }
      Err(why) => return Err(Failure::Failed(why.to_string())),
    };

    // The standing session writes the situation notes it owes before it is asked for its handoff.
    owed_notes(receiver, agent, &standing.id).await;

    let Some(handed) = handoff::ask(receiver, agent, &standing.id, chat_id).await else {
      return Ok(None);
    };

    crate::store::settings::set(
      receiver.db(),
      &handoff::pending_key(agent),
      &handed.path.to_string_lossy(),
    )
    .await
    .map_err(|why| Failure::Failed(why.detail))?;

    Ok(Some(handed))
  }

  /// A turn of a round handed to the agent's double (docs/drafting.md, 「A round handed to the double」), carried
  /// by a run of the double's role at the agent's scratch workspace: a slot of the role taken, the run's session
  /// started on it, the turn sent and waited to its end, and the slot given back. The round's own turn goes
  /// whole, the room's block and every document in full under the line that says the round was handed over; a
  /// later turn of the process goes as its words alone, since the session the role keeps holds the round. Words
  /// that come for the room meanwhile are pushed into the turn, and the turn's picture is drawn from its run.
  /// Nothing of the answer is the agent's own session's: it names no session, no context and no replacement.
  async fn doubled(
    &self,
    asking: &Asking,
    held: &Held,
    follower: &Follower,
    doubled: Doubled,
  ) -> Result<Reply, Failure> {
    let receiver = &self.receiver;
    let agency = receiver.units().agency.clone();
    let agent = held.agent.as_str();
    let chat_id = asking.chat_id;
    let thinking_shown = chat_id == receiver.owner_room();
    let failed = |why: crate::Fault| Failure::Failed(why.detail);
    let room = room_context(receiver, chat_id, asking.via.as_deref()).await.map_err(failed)?;
    let loaded = loaded_in(receiver, None).await.map_err(failed)?;
    let marked = states(&asking.prompt, &loaded);
    let (prompt, send) = match doubled {
      Doubled::Round => {
        let (sent, send, _) = sent_prompt(receiver, None, Some(&room), true, agent, &asking.prompt)
          .await
          .map_err(failed)?;

        (format!("{}\n\n{sent}", CHAT_LINE.room.handed), send)
      }
      Doubled::Later => (asking.prompt.clone(), None),
    };
    let folder = receiver.data_root().path().join("agent-workspaces").join(agent).join("scratch");

    std::fs::create_dir_all(&folder)
      .map_err(|why| Failure::Failed(format!("{} could not be made: {why}", folder.display())))?;

    let at = now_ms();
    let start = DoubleStart {
      supervisor: agent.to_owned(),
      path: folder.to_string_lossy().into_owned(),
      key: format!("{DOUBLE_SOURCE}:{agent}:{chat_id}:{at}"),
      run: format!("round-{agent}-{chat_id}-{at}"),
      title: format!("a round of chat {chat_id}"),
    };
    let seat = agency.double_start(&start).await.map_err(fault_of)?;
    let mut variables = serde_json::Map::new();

    variables.insert("BMO_TURN_CHAT".to_owned(), chat_id.to_string().into());
    variables.insert("BMO_TURN_PRIVILEGED".to_owned(), if asking.privileged { "1" } else { "0" }.into());
    variables.insert(
      "BMO_TURN_MESSAGES".to_owned(),
      asking.answers.iter().map(i64::to_string).collect::<Vec<_>>().join(",").into(),
    );

    if let Some(via) = &asking.via {
      variables.insert("BMO_TURN_VIA".to_owned(), via.clone().into());
    }

    let turn = DoubleTurn {
      prompt,
      title: format!("{agent}: a round of chat {chat_id}, handed over"),
      chat_id,
      variables,
    };
    let mut stream = match agency.events(&["turn.created"]).await {
      Ok(stream) => Some(stream),
      Err(why) => {
        eprintln!("chat {chat_id}: the agency's events were not read for the double's turn: {why}");

        None
      }
    };
    let port = agency.clone();
    let sent_on = seat.clone();
    let mut sending = tokio::spawn(async move { port.double_turn(&sent_on, &turn).await });
    let handed = handed_of(chat_id, send, &asking.context, &marked);
    let mut picture = RunProgress::starting(&handed);
    let mut turning: Option<i64> = None;
    let mut after: i64 = 0;
    let mut look = tokio::time::interval(self.look.max(Duration::from_millis(1)));
    let mut quiet_until: Option<tokio::time::Instant> = None;

    look.set_missed_tick_behavior(MissedTickBehavior::Delay);

    // The turn is waited to its end, however long it takes: a look offers it the words that came for its room
    // and reads its run's log for the picture, as a look at the agent's own turn does.
    let ended = loop {
      tokio::select! {
        ended = &mut sending => break ended,
        read = async { match stream.as_mut() { Some(stream) => stream.next().await, None => std::future::pending().await } } => {
          match read {
            Some(Ok(Event { kind, data, .. })) => {
              let mine = data.get("session").and_then(Value::as_str).is_some_and(|one| one == seat.session);

              if mine && kind == "turn.created" {
                turning = data.get("turnId").and_then(Value::as_i64);
              }
            }
            // A stream that ended or failed leaves the turn waited on with no more looks at its events.
            Some(Err(_)) | None => stream = None,
          }
        }
        _ = look.tick() => {
          if let Some(turning) = turning
            && quiet_until.is_none_or(|until| tokio::time::Instant::now() >= until)
          {
            quiet_until = pushed(agency.as_ref(), &seat.session, turning, follower).await;
          }

          let since = if after == 0 { None } else { Some(after) };

          if let Ok(lines) = agency.run_log(&seat.run, since).await {
            for line in lines {
              let id = line.get("id").and_then(Value::as_i64);

              if after != 0 && id.is_some_and(|id| id <= after) {
                continue;
              }

              let text = line
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| serde_json::to_string(&line).unwrap_or_default());

              if let Some(read) = picture.read(&text).or_else(|| picture.filed(&line)) {
                picture = read;
              }

              if thinking_shown && let Some(thought) = thinking_of(&line).last() {
                picture = picture.thought(thought);
              }

              after = id.unwrap_or(after);
            }

            follower.progressed(picture_of(&picture));
          }
        }
      }
    };

    drop(stream);

    let ended = match ended {
      Ok(Ok(ended)) => Ok(ended),
      Ok(Err(why)) => Err(fault_of(why)),
      Err(_) => Err(Failure::Failed("the double's turn went away".to_owned())),
    };
    let outcome = match &ended {
      Ok(ended) if !ended.outcome.is_empty() => ended.outcome.clone(),
      Ok(_) => "answered".to_owned(),
      Err(_) => "failed".to_owned(),
    };

    if let Err(why) = agency.double_finish(&seat, &outcome, false).await {
      eprintln!("chat {chat_id}: the double's slot was not given back: {why}");
    }

    let ended = ended?;

    match ended.outcome.as_str() {
      "stopped" => Err(Failure::Stopped),
      "failed" => Err(Failure::Failed(ended.reason)),
      // A double hands nothing on: the round was its to finish.
      _ if is_handoff(&ended.answer) => Err(Failure::Failed(
        "the double answered the hand-off sentinel instead of the round".to_owned(),
      )),
      _ => Ok(Reply {
        text: ended.answer,
        mode: None,
        account: None,
        session: None,
        context_tokens: None,
        context_window: None,
        fold_ratio: None,
        folds_itself: true,
        replaced: vec![],
        run: Some(seat.run.clone()),
        before: super::draft::Before {
          session: None,
          core: marked,
          names: super::context::core_names(&asking.prompt),
          loads: loaded,
        },
      }),
    }
  }

  pub async fn carries(&self, asking: &Asking, held: &Held, follower: &Follower) -> Result<Reply, Failure> {
    if let Some(doubled) = asking.double {
      return self.doubled(asking, held, follower, doubled).await;
    }

    let receiver = &self.receiver;
    let agency = receiver.units().agency.clone();
    let db = receiver.db();
    let agent = held.agent.as_str();
    let chat_id = asking.chat_id;
    // The picture of a turn working in the owner's private chat carries the run's last thinking block; no other
    // room's does (docs/messages.md, 「A working turn's thinking」).
    let thinking_shown = chat_id == receiver.owner_room();

    pace_first_launch(receiver, chat_id).await;

    let unified = unified_session(receiver, agent).await;
    let scope = scope_of(unified, chat_id);
    let started = agency.start(agent, scope).await.map_err(fault_of)?;
    let started_id = started.id.to_owned();

    let room = room_context(receiver, chat_id, asking.via.as_deref())
      .await
      .map_err(|why| Failure::Failed(why.detail))?;

    // A session the agency has just started is not opened until its first turn, so a state it answers
    // "not opened" for is the fresh session's, with no provider or model to resume yet.
    let read = agency.state(&started_id).await;
    let standing = read.clone().unwrap_or_default();
    let opened = Standing::of(&read);

    // The one session an agent keeps across its rooms, where its head or its road moved, is asked for its
    // handoff, let go here and started again, the note it left riding at the head of the new one's first turn
    // (docs/unified-bmo.md, 「Replacing the session」). A room's own session is the agency's to resume.
    let asked = Route {
      provider: standing.provider.as_deref().unwrap_or_default().to_owned(),
      model: standing.model.as_deref().unwrap_or_default().to_owned(),
      effort: String::new(),
      account: None,
      session: Some(started_id.clone()),
      transcript: None,
    };
    // A turn that goes on the session as it stands judges no replacement.
    let reason = if unified && !asking.standing {
      reset_reason(db, agent, &asked, &standing)
        .await
        .map_err(|why| Failure::Failed(why.detail))?
    } else {
      None
    };
    let replacing = reason.is_some();
    let (session, note, replaced): (Session, Option<String>, Vec<SessionReason>) = match reason {
      None => (started, None, vec![]),
      Some(reason) => {
        let again = self.replace(agent, &started_id, scope, chat_id, reason).await?;

        (again.0, Some(again.1), vec![reason])
      }
    };

    // A generation the agency opened under the session it handed over is no replacement of the receiver's: what
    // the ended generation held is released before this turn is composed, and the fold reported to the brain so
    // the marks it handed that generation clear (docs/unified-bmo.md, 「Replacing the session」). A session the
    // receiver replaced has been released already.
    let answered = if replacing {
      None
    } else {
      generation::answered_on(db, agent, &session.id).await.unwrap_or_else(|why| {
        eprintln!("{agent}: session {}'s generation was not read: {}", session.id, why.detail);

        None
      })
    };
    let expected = if replacing {
      false
    } else {
      generation::pending(db, agent, &session.id).await.unwrap_or_else(|why| {
        eprintln!("{agent}: session {}'s coming generation was not read: {}", session.id, why.detail);

        false
      })
    };
    let released = !replacing && (expected || generation::changed(answered, opened));

    if released {
      generation::release(receiver, agent, &session.id).await;

      if let Err(why) = generation::keep(db, agent, &session.id, opened.released()).await {
        eprintln!("{agent}: session {}'s generation was not written down: {}", session.id, why.detail);
      }
    } else if replacing && let Err(why) = generation::settle(db, agent, &session.id).await {
      eprintln!("{agent}: session {}'s coming generation was not settled: {}", session.id, why.detail);
    }

    let answered = if released { opened.released() } else { answered };
    // A handoff the agency asked for before it opened a fresh session is that session's note: the turn is the
    // fresh session's where the agency's state names it never opened, or names a generation it opened under the
    // session, which this turn releases; a turn on a session it resumed lets the kept handoff go
    // (docs/unified-bmo.md, 「Replacing the session」).
    let fresh = !standing.generation.as_i64().is_some_and(|generation| generation > 0) || released;
    let pending = handoff::take_pending(receiver, agent, fresh && note.is_none()).await;
    let note = note.or_else(|| pending.as_ref().map(carried_note));

    // The turn is composed for the session it goes on: a new one holds nothing yet, so the room's block and every
    // reference document go in full.
    let (sent, send, notes) =
      sent_prompt(receiver, Some(session.id.as_str()), Some(&room), true, agent, &asking.prompt)
        .await
        .map_err(|why| Failure::Failed(why.detail))?;
    let loaded = loaded_in(receiver, Some(session.id.as_str()))
      .await
      .map_err(|why| Failure::Failed(why.detail))?;
    let marked = states(&asking.prompt, &loaded);
    let before = super::draft::Before {
      session: Some(session.id.clone()),
      core: marked.clone(),
      names: super::context::core_names(&asking.prompt),
      loads: loaded,
    };
    // A new session's first turn opens on the handoff note, then the room's block with the brain's index at
    // session opening, then the turn.
    let prompt = match note {
      Some(note) => format!("{note}\n\n{sent}"),
      None => sent,
    };

    let turn = turn_of(asking, &session, &room, &prompt);
    let kinds: Vec<&'static str> = vec!["turn.created", "turn.ended", "system-prompt-changed"];
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
    let mut dropped = false;

    look.set_missed_tick_behavior(MissedTickBehavior::Delay);

    while ended.is_none() {
      // A look offers the working turn the words that came for its room and reads its run's log as an event
      // does, so the picture moves while the turn works between its events.
      let read = tokio::select! {
        read = stream.next() => Some(read),
        _ = look.tick() => {
          if let Some(turning) = turning
            && quiet_until.is_none_or(|until| tokio::time::Instant::now() >= until)
          {
            quiet_until = pushed(agency.as_ref(), &session.id, turning, follower).await;
          }

          None
        }
      };

      let looked = read.is_none();

      match read {
        None => {}
        Some(None) => {
          dropped = true;
          break;
        }
        Some(Some(Ok(Event { kind, data, .. }))) => {
          let body = &data;
          let mine = body.get("session").and_then(Value::as_str).is_some_and(|one| one == session.id);

          if mine && kind == "turn.created" {
            run = body.get("run").and_then(Value::as_str).map(str::to_owned);
            turning = body.get("turnId").and_then(Value::as_i64);
          } else if mine && kind == "turn.ended" {
            ended = Some(body.clone());
          } else if mine
            && kind == "system-prompt-changed"
            && let Err(why) = generation::expect(db, agent, &session.id).await
          {
            eprintln!(
              "{agent}: session {}'s coming generation was not written down: {}",
              session.id, why.detail
            );
          }
        }
        Some(Some(Err(why))) => return Err(fault_of(why)),
      }

      if let Some(run) = &run {
        let since = if after == 0 { None } else { Some(after) };

        // A look's read that failed leaves the picture as it stands until the next; an event's ends the turn.
        let lines = match agency.run_log(run, since).await {
          Ok(lines) => lines,
          Err(why) if looked => {
            eprintln!("session {}: the run's log was not read for its picture: {why}", session.id);
            continue;
          }
          Err(why) => return Err(fault_of(why)),
        };

        for line in lines {
          let id = line.get("id").and_then(Value::as_i64);

          // A line already read changes nothing, so a log that answers more than was asked reads each once.
          if after != 0 && id.is_some_and(|id| id <= after) {
            continue;
          }

          let text = line
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| serde_json::to_string(&line).unwrap_or_default());

          // A line holding a record or an event is read as that; one the agency filed already told apart, as what
          // it names.
          if let Some(read) = picture.read(&text).or_else(|| picture.filed(&line)) {
            picture = read;
          }

          if thinking_shown && let Some(thought) = thinking_of(&line).last() {
            picture = picture.thought(thought);
          }

          after = id.unwrap_or(after);
        }

        follower.progressed(picture_of(&picture));
      }
    }

    drop(stream);

    let delivered =
      sending.await.map_err(|_| Failure::Failed("the turn's carrying went away".to_owned()))?;
    let delivered = delivered.map_err(fault_of)?;

    // A turn whose end the stream never brought, because the stream ended first, is read from the agency; a turn
    // it no longer holds has failed, and the round with it (docs/bmo-window.md, 「Quiet instruction」).
    if ended.is_none() && dropped {
      let lost = "the agency's event stream ended before the turn did";
      let Some(turning) = turning else {
        return Err(Failure::Failed(format!("{lost}, and the turn was never named")));
      };

      match agency.turn_read(&session.id, turning).await {
        Ok(read) if read.ended() => {
          ended = Some(serde_json::json!({ "outcome": read.outcome, "answer": read.answer }));
        }
        Ok(_) => return Err(Failure::Failed(format!("{lost}, and the agency says it has not ended"))),
        Err(why) => return Err(Failure::Failed(format!("{lost}, and the agency does not hold it: {why}"))),
      }
    }

    // A session's first turn opens it as the agency carries the turn; a state it has not answered for
    // yet is read as the fresh session's, with nothing of a request behind it.
    let ended_read = agency.state(&session.id).await;
    let standing = match &ended_read {
      Ok(standing) => standing.clone(),
      Err(why) => {
        eprintln!("chat {chat_id}: the state of session {} was not read after its turn: {why}", session.id);
        Default::default()
      }
    };

    // The generation the session answered on is kept for the next turn: one the agency opened while this turn
    // ran is the next turn's to release (docs/unified-bmo.md, 「Replacing the session」).
    let marker = generation::after(answered, opened, Standing::of(&ended_read).at(), replacing);

    if let Err(why) = generation::keep(db, agent, &session.id, marker).await {
      eprintln!("{agent}: session {}'s generation was not written down: {}", session.id, why.detail);
    }

    let ended: Value = ended.clone().unwrap_or(Value::Null);
    let answer = ended.get("answer").and_then(Value::as_str).unwrap_or_default();
    let outcome = ended.get("outcome").and_then(Value::as_str).unwrap_or_default();

    if delivered == Delivery::Delivered {
      let resumed = replaced.is_empty().then_some(started_id.as_str());
      let _ = remember_held(
        receiver,
        resumed,
        &session.id,
        Some(&room),
        true,
        &asking.prompt,
        asking.thread_mark.clone(),
      )
      .await;

      for note in &notes {
        if let Err(why) = super::situation::keep(db, &session.id, note).await {
          eprintln!("chat {chat_id}: the situation note's version was not kept: {}", why.detail);
        }
      }

      let _ = remember(
        db,
        agent,
        &Leaving {
          chat_id,
          route: Route {
            provider: standing.provider.as_deref().unwrap_or_default().to_owned(),
            model: standing.model.as_deref().unwrap_or_default().to_owned(),
            effort: String::new(),
            account: None,
            session: Some(session.id.to_owned()),
            transcript: None,
          },
          moved: standing.prefix_change_reason.clone(),
          process: room.process.clone(),
          resumed: replaced.is_empty().then_some(started_id),
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
        fold_ratio: standing.fold_ratio,
        folds_itself: standing.folds_itself,
        replaced: replaced.clone(),
        run: run.clone(),
        before,
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

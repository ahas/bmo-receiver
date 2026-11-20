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

use serde_json::Value;
use tokio::time::MissedTickBehavior;

use receiver_codes::SessionReason;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::handover::HANDOVER;
use receiver_ports::agency::{Agency, Delivery, Event, Said, Scope, Session, Turn, TurnRoom};
use receiver_ports::http::PortFailure;

use super::attach::{Followed, Probed, Step};
use super::cache::{
  Leaving, Route, active_route, owe_replacement, owed_replacement, remember, reset_reason,
  settle_replacement, spend_force, waits_for_quiet,
};
use super::context::{RoomSend, State, loaded_in, remember_held, sent_prompt, states};
use super::conversation::{
  Aside, Asking, BoxFuture, Carrying, Failure, Follower, Held, Reply, unified_session,
};
use super::generation::{self, Standing};
use super::handoff::{self, Note, carried_note};
use super::progress::RunProgress;
use super::room::{RoomContext, room_context};
use crate::now_ms;
use crate::receiver::Receiver;
use crate::room_record::RoomForget;
use crate::rounds;
use crate::run_lines::thinking_of;

// The least a room's first launch since the receiver started is spaced from another room's: several sessions
// starting from cold at once is the shape of abuse.
const LAUNCH_GAP: Duration = Duration::from_secs(10);

static OPENED: Mutex<u64> = Mutex::new(0);

// The events a round's turn is followed by.
const TURN_KINDS: &[&str] = &["turn.created", "turn.ended", "system-prompt-changed"];

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

/// The replacement an agent's one session owes made while the agent is quiet (docs/unified-bmo.md, 「Replacing the
/// session」): the session writes the notes it owes and its handoff, is ended and started again, and the note the
/// fresh session's first turn opens on is kept for it. True where a replacement was owed and made; the caller holds
/// the agent's turn, so words that arrive while it is made wait for the fresh session.
pub async fn replace_owed(receiver: &Receiver, agent: &str) -> bool {
  let db = receiver.db();
  let reason = match owed_replacement(db, agent).await {
    Ok(Some(reason)) => reason,
    Ok(None) => return false,
    Err(why) => {
      eprintln!("{agent}: the replacement owed was not read: {}", why.detail);

      return false;
    }
  };
  let agency = receiver.units().agency.clone();
  let chat_id = receiver.owner_room();
  let scope = scope_of(unified_session(receiver, agent).await, chat_id);
  let old = match agency.start(agent, scope).await {
    Ok(standing) => standing.id,
    Err(why) => {
      eprintln!("{agent}: the session owing a replacement ({reason}) was not reached: {why}");

      return false;
    }
  };

  // A session no turn of the receiver's went on holds nothing to hand over: the debt is settled as it stands.
  let turned = active_route(db, agent)
    .await
    .ok()
    .flatten()
    .is_some_and(|route| route.session.as_deref() == Some(&old));

  if !turned {
    if let Err(why) = spend_force(db, agent).await {
      eprintln!("{agent}: the word to start afresh was not spent: {}", why.detail);
    }

    if let Err(why) = settle_replacement(db, agent).await {
      eprintln!("{agent}: the replacement owed was not settled: {}", why.detail);
    }

    println!(
      "{agent}: session {old} owed a replacement ({reason}) and no turn had gone on it; nothing to hand over"
    );

    return true;
  }

  receiver.replacing_begins(agent);
  owed_notes(receiver, agent, &old).await;

  let handed = handoff::ask(receiver, agent, &old, chat_id).await;

  if let Err(why) = agency.end(&old).await {
    eprintln!("{agent}: session {old} was not ended for its replacement ({reason}): {why}");
    receiver.replacing_ends(agent);

    return false;
  }

  generation::forget(receiver, agent, &old).await;

  if let Err(why) = spend_force(db, agent).await {
    eprintln!("{agent}: the word to start afresh was not spent: {}", why.detail);
  }

  if let Err(why) = settle_replacement(db, agent).await {
    eprintln!("{agent}: the replacement owed was not settled: {}", why.detail);
  }

  let note = match handed {
    Some(handed) => Note::Handed(handed),
    None => handoff::fallback(receiver, agent, None, reason).await,
  };

  handoff::keep_pending(receiver, agent, &note).await;

  // The fresh session stands ready for the next turn, so no question waits for its start.
  if let Err(why) = agency.start(agent, scope).await {
    eprintln!("{agent}: the fresh session was not started after the replacement ({reason}): {why}");
  }

  receiver.replacing_ends(agent);

  if let Some(times) = handoff::in_a_row(receiver, agent, &format!("replacement-{reason}"), true).await {
    eprintln!(
      "{agent}: session replaced again for the same reason ({reason}), {times} in a row, each within {} minutes of \
       the last",
      HANDOVER.again_minutes
    );
  }

  println!(
    "{agent}: session {old} replaced while the agent was quiet ({reason}), the next one opening on {}",
    match &note {
      Note::Handed(handed) => handed.path.display().to_string(),
      Note::Carried(_) => "the transcript".to_owned(),
      Note::Lost => "nothing recovered".to_owned(),
    }
  );

  true
}

/// The end of the turn holding the session, heard on the round's stream while its own turn was refused; a
/// re-attachment that finds no turn named ends the wait as well, and the send made again tells whether the session
/// is free.
async fn other_turn_ended(followed: &mut Followed) -> Result<(), Failure> {
  loop {
    match followed.next().await? {
      Step::Ended(_) | Step::Unnamed => return Ok(()),
      Step::Event(_) => {}
    }
  }
}

/// Why the agency's answer to a session operation ended the turn: uncounted where the agency did not answer.
fn fault_of(why: PortFailure) -> Failure {
  super::attach::failure_of(why)
}

/// The carrying's own key: the round, the room and the moment the turn took its words, so a send made again after a
/// cut attaches to the turn standing under it.
fn key_of(asking: &Asking) -> String {
  format!(
    "{}:{}:{}",
    rounds::current().unwrap_or_else(|| "turn".to_owned()),
    asking.chat_id,
    asking.began
  )
}

/// The number of the last of the agency's events the receiver heard, from which a turn sent now is followed.
async fn heard_mark(receiver: &Receiver) -> Option<i64> {
  use crate::hearing::Marks;

  crate::hearing::HeardSetting(receiver.db().clone()).last().await.map(|mark| mark.id)
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
    round: Some(rounds::of_turn(asking.chat_id)),
    key: Some(key_of(asking)),
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
    receiver.replacing_begins(agent);
    owed_notes(receiver, agent, old).await;

    let handed = handoff::ask(receiver, agent, old, chat_id).await;

    if let Err(why) = agency.end(old).await {
      receiver.replacing_ends(agent);

      return Err(fault_of(why));
    }

    // The ended session's loads, its room's block and its cache record go with it; the one started next holds
    // nothing yet.
    generation::forget(receiver, agent, old).await;

    if let Err(why) = spend_force(db, agent).await {
      eprintln!("{agent}: the word to start afresh was not spent: {}", why.detail);
    }

    let note = match handed {
      Some(handed) => Note::Handed(handed),
      None => handoff::fallback(receiver, agent, None, reason).await,
    };
    let again = agency.start(agent, scope).await;

    receiver.replacing_ends(agent);

    let again = again.map_err(fault_of)?;

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
    receiver.replacing_begins(agent);
    owed_notes(receiver, agent, &standing.id).await;

    let handed = handoff::ask(receiver, agent, &standing.id, chat_id).await;

    receiver.replacing_ends(agent);

    // The session asked wrote nothing: what the receiver keeps of its answers stands in for the fresh session's
    // first turn, or the word that nothing was recovered, the owner told once.
    let Some(handed) = handed else {
      let note = handoff::fallback(receiver, agent, None, SessionReason::Asked).await;

      handoff::keep_pending(receiver, agent, &note).await;

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

  pub async fn carries(&self, asking: &Asking, held: &Held, follower: &Follower) -> Result<Reply, Failure> {
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

    let room = room_context(receiver, agent, chat_id, asking.via.as_deref())
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
      session: Some(started_id.clone()),
      transcript: None,
    };
    // A turn that goes on the session as it stands judges no replacement.
    let judged = unified && !asking.standing;
    let reason = if judged {
      reset_reason(db, agent, &asked, &standing)
        .await
        .map_err(|why| Failure::Failed(why.detail))?
    } else {
      None
    };
    // A replacement never stands between a question and its answer: for a reason the session can still take the
    // turn under, the turn goes on in the session as it stands and the replacement is owed, to be made once the
    // agent is quiet (docs/unified-bmo.md, 「Replacing the session」).
    let reason = match reason {
      Some(reason) if waits_for_quiet(reason) => {
        match owe_replacement(db, agent, reason).await {
          Ok(true) => println!(
            "{agent}: session {started_id} owes a replacement ({reason}); this turn goes on in it, and the session \
             is replaced once the agent is quiet"
          ),
          Ok(false) => {}
          Err(why) => {
            eprintln!("{agent}: the replacement owed ({reason}) was not written down: {}", why.detail)
          }
        }

        None
      }
      other => other,
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
    // A session the agency opened without the handoff it was owed opens on what the receiver keeps of its answers
    // — a room's own session on that room's — or on the word that nothing was recovered (docs/unified-bmo.md,
    // 「Replacing the session」).
    let lost = match &note {
      None if standing.handoff_lost => {
        Some(handoff::fallback(receiver, agent, (!unified).then_some(chat_id), SessionReason::Asked).await)
      }
      _ => None,
    };
    let note = note.or_else(|| lost.as_ref().map(carried_note));

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
    let mark = heard_mark(receiver).await;
    let mut followed =
      Followed::open(agency.clone(), &session.id, TURN_KINDS, mark, receiver.re_attach_pace()).await;

    // The turn is sent apart, so the room's picture of it can be drawn from the run's log while it works; a send
    // whose connection is cut is no end of the turn, which the round follows on and re-attaches to.
    let sent = turn.clone();
    let port = agency.clone();
    // From its send to its end the round waits on the agency: a handoff the agency asks meanwhile, ahead of the
    // fresh session this turn goes on, is not held behind the round (docs/unified-bmo.md, 「Replacing the session」).
    let at_agency = receiver.waits_on_agency(agent);
    let mut sending = tokio::spawn(async move { port.turn(&sent).await });
    let mut sent_done = false;
    let mut delivered: Option<Delivery> = None;
    let mut carrying: Option<i64> = None;

    let handed = handed_of(chat_id, send, &asking.context, &marked);
    let mut picture = RunProgress::starting(&handed);
    let mut after: i64 = 0;
    let mut ended: Option<Value> = None;
    // An end heard while the send is unanswered and the round's turn unnamed, kept aside until the send says
    // whose it was: the round's own where the send went in, the turn holding the session where it was refused.
    let mut early: Option<Value> = None;
    let mut look = tokio::time::interval(self.look.max(Duration::from_millis(1)));
    let mut quiet_until: Option<tokio::time::Instant> = None;

    look.set_missed_tick_behavior(MissedTickBehavior::Delay);

    while ended.is_none() {
      // A look offers the working turn the words that came for its room and reads its run's log as an event
      // does, so the picture moves while the turn works between its events.
      let looked = tokio::select! {
        step = followed.next(), if early.is_none() => {
          match step? {
            Step::Ended(end) if !sent_done && followed.turn().is_none() => early = Some(end),
            Step::Ended(end) => ended = Some(end),
            Step::Event(Event { kind, .. }) => {
              if kind == "system-prompt-changed"
                && let Err(why) = generation::expect(db, agent, &session.id).await
              {
                eprintln!(
                  "{agent}: session {}'s coming generation was not written down: {}",
                  session.id, why.detail
                );
              }
            }
            // No turn of the session was named since the send went out: a send whose connection was cut goes
            // again under its key, and one still waiting is waited on. A send that answered names a turn that
            // ended with no end the round could read, so the round fails, uncounted.
            Step::Unnamed if delivered.is_some() => {
              return Err(Failure::Transport(format!(
                "session {}'s turn ended with no event of it the agency still keeps",
                session.id
              )));
            }
            Step::Unnamed => {
              if sent_done {
                println!("session {}: its turn was never named, so it is sent again under its key", session.id);

                let sent = turn.clone();
                let port = agency.clone();

                sending = tokio::spawn(async move { port.turn(&sent).await });
                sent_done = false;
              }
            }
          }

          false
        }
        done = &mut sending, if !sent_done => {
          sent_done = true;

          match done {
            Ok(Ok(delivery)) => {
              delivered = Some(delivery);
              ended = early.take();
            }
            // The session is in another turn, a handoff's, a replacement's or one the agency runs of its own: the
            // round waits for that turn's end and sends its own again, counting nothing (docs/bmo-window.md,
            // 「Rounds」). The stream is opened anew past that end, so it is never read as the round's own.
            Ok(Err(why)) if handoff::busy(&why) => {
              eprintln!(
                "session {}: its turn was refused, the session being in another turn; the round waits for that \
                 turn to end and sends its own again: {why}",
                session.id
              );

              if early.take().is_none() {
                other_turn_ended(&mut followed).await?;
              }

              let past = followed.mark().or(mark);

              followed =
                Followed::open(agency.clone(), &session.id, TURN_KINDS, past, receiver.re_attach_pace()).await;

              let sent = turn.clone();
              let port = agency.clone();

              sending = tokio::spawn(async move { port.turn(&sent).await });
              sent_done = false;
            }
            Ok(Err(why)) if super::attach::unreachable(&why) => {
              eprintln!("session {}: the turn's send was cut: {why}", session.id);
              ended = early.take();
              followed.cut();
            }
            Ok(Err(why)) => return Err(fault_of(why)),
            Err(_) => {
              ended = early.take();
              followed.cut();
            }
          }

          false
        }
        _ = look.tick() => {
          let turning = followed.turn();
          let asked = turning.is_some() && quiet_until.is_none_or(|until| tokio::time::Instant::now() >= until);
          let mut offered = Offered::default();

          if asked {
            offered = pushed(agency.as_ref(), &session.id, turning.unwrap(), follower).await;
            quiet_until = offered.quiet_until;
          }

          // The agency no longer works the turn and the stream told no end of it: the round re-attaches and
          // reads the end the agency kept. A push that went in answers that the turn stands going; a look
          // that carried no words reads the turn itself, whose end the read carries moves the round on as
          // the stream's own end would, and a turn read gone, or a send answered whose turn no event ever
          // named, cuts the following — its re-attachment may still give the end whole off the events kept.
          let gone = match turning {
            Some(_) if offered.turn_gone => true,
            Some(_) if offered.unasked => match followed.probe().await {
              Probed::Ended(end) => {
                ended = Some(end);

                false
              }
              Probed::Gone => true,
              Probed::Going => false,
            },
            None => delivered.is_some(),
            _ => false,
          };

          if gone {
            match (turning, delivered.is_some()) {
              (Some(turning), _) => eprintln!(
                "session {}: turn {turning} is no longer going and no end of it was heard; re-attaching to read it",
                session.id
              ),
              (None, true) => eprintln!(
                "session {}: its send answered and no turn of it was ever named; re-attaching to read what stands",
                session.id
              ),
              (None, false) => {}
            }

            followed.cut();
          }

          true
        }
      };

      // The turn carrying the round, once the agency names it, carries the rows its prompt holds.
      if followed.turn().is_some() && followed.turn() != carrying {
        carrying = followed.turn();

        if let Some(turning) = carrying {
          follower.carried_by(&session.id, turning).await;
        }
      }

      if ended.is_some() {
        break;
      }

      if let Some(run) = followed.run() {
        let since = if after == 0 { None } else { Some(after) };

        // A read that failed leaves the picture as it stands until the next look.
        let lines = match agency.run_log(&run, since).await {
          Ok(lines) => lines,
          Err(why) => {
            if looked {
              eprintln!("session {}: the run's log was not read for its picture: {why}", session.id);
            }

            continue;
          }
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

        // The block still being written, which no line says yet, read after the lines that may have said it.
        let open = agency.open_block(&session.id).await.ok().flatten();

        picture = picture.with_open(open.as_ref().map(|block| (block.kind.as_str(), block.bytes)));
        follower.progressed(&picture);
      }
    }

    let run = followed.run();

    drop(followed);
    drop(at_agency);

    // The send answers once the turn has ended; one whose connection was cut leaves the turn's delivery to what
    // its end says, the words taken where it names nothing else.
    if !sent_done {
      match sending.await {
        Ok(Ok(delivery)) => delivered = Some(delivery),
        Ok(Err(why)) if !super::attach::unreachable(&why) && delivered.is_none() => {
          return Err(fault_of(why));
        }
        _ => {}
      }
    }

    let delivered = delivered
      .or_else(|| {
        ended
          .as_ref()
          .and_then(|end| end.get("delivery"))
          .and_then(|delivery| serde_json::from_value(delivery.clone()).ok())
      })
      .unwrap_or(Delivery::Delivered);

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
    let built = ended.get("answer").and_then(Value::as_str).unwrap_or_default();
    // A later turn of the program that says nothing leaves the answer written before it (docs/drafting.md,
    // 「What the receiver reads of an answer」).
    let spanned = super::draft::Spanned::of(&ended);
    let answer = super::draft::spanned_answer(built, &spanned);
    let answer = answer.as_str();
    let spanned = if spanned.len() > 1 { spanned } else { vec![] };
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
            session: Some(session.id.to_owned()),
            transcript: None,
          },
          moved: standing.prefix_change_reason.clone(),
          process: room.process.clone(),
          judged,
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
        spanned,
      }),
    }
  }
}

/// What became of an offer of the words that came for the round's room while its turn works: the moment before
/// which nothing more is offered, where a refusal or a push that failed asks for that gap, whether the agency
/// answered that no such turn is going, and whether the look had no words at all and therefore asked the agency
/// nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Offered {
  pub quiet_until: Option<tokio::time::Instant>,
  pub turn_gone: bool,
  pub unasked: bool,
}

/// The words that came for the round's room while its turn works, offered to the turn and settled on what the
/// agency said became of them; a look with no words asks the agency nothing, and `unasked` says so.
pub async fn pushed(agency: &dyn Agency, session: &str, turn: i64, aside: &dyn Aside) -> Offered {
  let Some(words) = aside.offer().await else {
    return Offered {
      unasked: true,
      ..Offered::default()
    };
  };
  let mut turn_gone = false;
  let said = match agency.push(session, turn, &words).await {
    Ok(said) => said,
    Err(why) => {
      eprintln!("session {session}: words for the working turn did not go in: {why}");
      turn_gone = matches!(why, PortFailure::Answered { status: 404, .. });
      Said::Refused
    }
  };

  aside.settle(said.into()).await;

  Offered {
    quiet_until: (said == Said::Refused)
      .then(|| tokio::time::Instant::now() + Duration::from_millis(CHAT_ROUND.pushing.refused_gap_ms)),
    turn_gone,
    unasked: false,
  }
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

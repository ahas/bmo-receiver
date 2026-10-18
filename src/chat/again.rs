//! The round's second askings (docs/answer-checks.md): the checks an answer is held to before it goes out — an
//! owed answer over a skip, the room's language, a promise with nothing recorded, a promise with nothing set to
//! wake, a rule the owner set, a word the owner banned — each asking the same session once more for this same
//! turn (twice at most for the promises), in the order the round calls them.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::to_string as stringify;

use receiver_policy::fill_marks;
use receiver_policy::second_asks::SECOND_ASKS;

use super::ask::Carry;
use super::conversation::{
  Asking, BoxFuture, Carrying, Checked, Conversations, Failure, Follower, Held, Reply,
};
use super::promises::{answered_in_the_wrong_language, promises_after, promises_later, sets_rule_later};
use super::prompt::{Scope, scope_line};
use super::records::run_records;
use super::sentinels::{is_skip, is_wait};
use super::turnover::{judge, note_session, verdict_of};
use super::window::{Selection, composed};
use crate::form::strip_prompt_scaffolding;
use crate::now_ms;
use crate::receiver::{Receiver, Room};
use crate::store::Fault;
use crate::wording::{BannedTerm, banned_terms_in, banned_terms_line, read_glossary};

/// Which second asking a run of the session was, as its judgement is kept under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
  Owed,
  Language,
  Record,
  Wake,
  Rule,
  Wording,
}

impl Stage {
  pub fn word(self) -> &'static str {
    match self {
      Self::Owed => "owed-again",
      Self::Language => "language-again",
      Self::Record => "record-again",
      Self::Wake => "wake-again",
      Self::Rule => "rule-again",
      Self::Wording => "wording-again",
    }
  }
}

/// How a run of a second asking ended, in the words its judgement is written with: a stop is somebody ending the
/// run, a timeout is a road that gave up waiting on its service, in its own words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
  Stopped,
  Timeout,
  Error,
}

impl Ending {
  pub fn word(self) -> &'static str {
    match self {
      Self::Stopped => "STOPPED",
      Self::Timeout => "TIMEOUT",
      Self::Error => "FAILED",
    }
  }
}

static TIMED_OUT: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?i)time(?:d)?[ -]?out").expect("a timeout"));

/// How a run that failed ended, read off what its road said.
pub fn ending_of(failure: &Failure) -> Ending {
  let said = match failure {
    Failure::Stopped => return Ending::Stopped,
    Failure::Limit | Failure::Outage => String::new(),
    Failure::Transport(message)
    | Failure::Failed(message)
    | Failure::Engine {
      reason: message, ..
    } => message.clone(),
  };

  if TIMED_OUT.is_match(&said) { Ending::Timeout } else { Ending::Error }
}

/// What a second asking goes out with beside its errand: the turn's scope, the messages waiting for the next turn,
/// and that they are not acted on now.
pub fn words(errand: &str, scope: &Scope, pending: &[i64]) -> String {
  format!(
    "{errand}\n{}\n{}{}\n{}",
    scope_line(scope),
    SECOND_ASKS.next_turn,
    stringify(&pending).unwrap_or_default(),
    SECOND_ASKS.later,
  )
}

/// What a second asking goes out with: the room's standing and road, the round's reference, the messages waiting
/// for the next turn, and whether the room's tools are on.
pub struct Asker<'a> {
  pub room: &'a Room,
  pub via: &'a str,
  pub privileged: bool,
  pub tools_allowed: bool,
  pub selection: &'a Selection,
  pub pending: &'a [i64],
}

/// One of the turn's messages as the rule check reads it: who said it and what.
pub struct Said {
  pub sender_id: Option<i64>,
  pub text: String,
}

/// A turn's answer as the second askings read it and change it: the answer standing, every run of the session the
/// round has made so far by the run its log went under, what the turn is held to, the room's recent words,
/// whether the turn's tools are on and the room's words the round took.
pub struct Turn {
  pub reply: Reply,
  pub spoken: Vec<String>,
  pub scope: Scope,
  pub recent: Vec<String>,
  pub tools_allowed: bool,
  pub batch: Vec<Said>,
}

/// Where a second asking stands: the conversations carrying the turn, the receiver, the agent and the room, the
/// session the turn goes on in, and what the asking goes out with.
pub struct Ctx<'a> {
  pub conversations: &'a Conversations,
  pub receiver: &'a Receiver,
  pub agent: &'a str,
  pub chat_id: i64,
  pub held: &'a Held,
  pub asker: &'a Asker<'a>,
}

/// A run of the session asked once more: none where it failed, the stop raised where it was stopped.
pub type Again = Result<Option<(Reply, Option<String>)>, Failure>;

/// The session resumed in its own conversation for one more run of this turn: what it says is written down as a
/// turn's answer is, under the asking's own stage.
pub async fn speak_again(ctx: &Ctx<'_>, stage: Stage, errand: &str, scope: &Scope) -> Again {
  let stage_word = stage.word();
  let chat = ctx.chat_id;
  let started = now_ms();
  let asking = Asking {
    chat_id: chat,
    session: ctx.held.session.clone().unwrap_or_default(),
    prompt: composed(ctx.asker.selection, &words(errand, scope, ctx.asker.pending)),
    via: match ctx.asker.via {
      "" => None,
      road => Some(road.to_owned()),
    },
    privileged: ctx.asker.privileged,
    answers: vec![],
    began: started,
    context: ctx.asker.selection.context.clone(),
  };
  let nap = Held {
    agent: ctx.agent.to_owned(),
    chat_id: chat,
    session: ctx.held.session.clone(),
    account: ctx.held.account.clone(),
    context_tokens: ctx.held.context_tokens,
    all_from_owner: ctx.asker.privileged,
    replaced: vec![],
    last_consumed_id: ctx.held.last_consumed_id,
  };
  let follower = Follower::new(ctx.conversations, chat);
  let carrying = Carry::new(ctx.receiver.clone());

  match carrying.carry(&asking, &nap, &follower).await {
    Ok(answered) => {
      let text = strip_prompt_scaffolding(&answered.text);
      let answer = Reply {
        text,
        ..answered.clone()
      };
      let ms = now_ms() - started;

      if let Err(why) = written(ctx, stage, &answer, ms).await {
        judge(
          ctx.receiver,
          chat,
          ctx.held.last_consumed_id,
          stage_word,
          Ending::Error.word(),
          None,
          Some(ms),
        )
        .await;
        eprintln!("chat {chat}: {stage_word} failed: {}", why.detail);

        return Ok(None);
      }

      Ok(Some((answer, answered.run)))
    }
    Err(failure) => {
      let ms = now_ms() - started;

      judge(
        ctx.receiver,
        chat,
        ctx.held.last_consumed_id,
        stage_word,
        ending_of(&failure).word(),
        None,
        Some(ms),
      )
      .await;

      if matches!(failure, Failure::Stopped) {
        return Err(failure);
      }

      eprintln!("chat {chat}: {stage_word} failed: {failure:?}");

      Ok(None)
    }
  }
}

// What a second asking leaves: the session the room holds and its verdict, under the asking's own stage.
async fn written(ctx: &Ctx<'_>, stage: Stage, answer: &Reply, ms: i64) -> Result<(), Fault> {
  note_session(
    ctx.receiver,
    ctx.agent,
    ctx.chat_id,
    ctx.held.all_from_owner,
    answer.session.as_deref(),
    ctx.asker.tools_allowed,
  )
  .await?;

  judge(
    ctx.receiver,
    ctx.chat_id,
    ctx.held.last_consumed_id,
    stage.word(),
    &verdict_of(&answer.text),
    Some(&answer.text),
    Some(ms),
  )
  .await;

  Ok(())
}

/// An answer that says something: not empty, not a skip and not a wait.
pub fn usable(text: &str) -> bool {
  !text.is_empty() && !is_skip(text) && !is_wait(text)
}

pub fn in_room_language(text: &str, recent: &[String]) -> bool {
  usable(text) && !answered_in_the_wrong_language(text, recent.iter().map(String::as_str))
}

// Asks the session once more and hands back its answer where it gave a usable one. The run joins the round's
// others whatever it answered, so what it wrote is seen by the check that follows.
async fn ask_again(
  ctx: &Ctx<'_>,
  turn: &mut Turn,
  stage: Stage,
  errand: &str,
) -> Result<Option<Reply>, Failure> {
  let Some((answer, run)) = speak_again(ctx, stage, errand, &turn.scope).await? else {
    return Ok(None);
  };

  turn.spoken.extend(run);

  Ok(usable(&answer.text).then_some(answer))
}

// Whether the round's runs wrote something down, of one kind where one is named; a watch is not among what any
// kind means, since it holds nothing and only wakes. One that cannot be read reads as nothing written.
pub async fn recorded(receiver: &Receiver, agent: &str, runs: &[String], kind: Option<&str>) -> bool {
  match run_records(receiver, agent, runs).await {
    Ok(records) => records
      .iter()
      .any(|record| kind.map_or(record.kind != "watch", |kind| record.kind == kind)),
    Err(why) => {
      eprintln!("the turn's records could not be read: {}", why.detail);

      false
    }
  }
}

// Whether the round's runs set something that wakes the agent.
async fn wakes(receiver: &Receiver, agent: &str, runs: &[String]) -> bool {
  match run_records(receiver, agent, runs).await {
    Ok(records) => records.iter().any(|record| record.wakes),
    Err(why) => {
      eprintln!("the turn's records could not be read: {}", why.detail);

      false
    }
  }
}

pub fn record_errand(tools: bool) -> String {
  let errands = &SECOND_ASKS.errands;

  format!("{}{}", errands.record, if tools { &errands.record_on } else { &errands.record_off })
}

pub fn wake_errand(tools: bool, chat_id: i64) -> String {
  let errands = &SECOND_ASKS.errands;
  let form = if tools {
    fill_marks(&errands.wake_on, &[("chat", &chat_id.to_string())])
  } else {
    errands.wake_off.clone()
  };

  format!("{}{form}", errands.wake)
}

/// Silence over the owner's own words is not the session's to choose: a turn that carried words owed an answer and
/// answered with a skip is asked once more, with the silence taken away.
pub async fn owed(ctx: &Ctx<'_>, turn: &mut Turn, owed: usize) -> Result<(), Failure> {
  if owed > 0 && is_skip(&turn.reply.text) {
    let errand = fill_marks(&SECOND_ASKS.errands.owed, &[("count", &owed.to_string())]);

    if let Some(again) = ask_again(ctx, turn, Stage::Owed, &errand).await? {
      turn.reply = again.clone();
    }
  }

  Ok(())
}

/// An answer in the machine's language in a room that speaks another is written once more in the room's; the
/// second writing stands where it is in the room's language.
pub async fn language(ctx: &Ctx<'_>, turn: &mut Turn) -> Result<(), Failure> {
  if answered_in_the_wrong_language(&turn.reply.text, turn.recent.iter().map(String::as_str)) {
    let errand = fill_marks(&SECOND_ASKS.errands.language, &[("what", &SECOND_ASKS.errands.answer)]);
    let again = ask_again(ctx, turn, Stage::Language, &errand).await?;

    if let Some(again) = again.filter(|again| in_room_language(&again.text, &turn.recent)) {
      turn.reply = again.clone();
    }
  }

  Ok(())
}

/// A promise about what the agent will do from now on stands only where this same turn wrote it down: one with
/// nothing written goes back to record it, and where it promises again with nothing written, once more with the
/// tools off to take the promise out.
pub async fn record(ctx: &Ctx<'_>, turn: &mut Turn) -> Result<(), Failure> {
  if promises_later(&turn.reply.text) && !recorded(ctx.receiver, ctx.agent, &turn.spoken, None).await {
    if let Some(again) = ask_again(ctx, turn, Stage::Record, &record_errand(turn.tools_allowed)).await? {
      turn.reply = again.clone();
    }

    if turn.tools_allowed
      && promises_later(&turn.reply.text)
      && !recorded(ctx.receiver, ctx.agent, &turn.spoken, None).await
      && let Some(last) = ask_again(ctx, turn, Stage::Record, &record_errand(false)).await?
    {
      turn.reply = last.clone();
    }
  }

  Ok(())
}

/// A promise to act once something has happened stands only where this same turn set what wakes the agent when it
/// does, asked for as a record is.
pub async fn wake(ctx: &Ctx<'_>, turn: &mut Turn) -> Result<(), Failure> {
  if promises_after(&turn.reply.text) && !wakes(ctx.receiver, ctx.agent, &turn.spoken).await {
    if let Some(again) =
      ask_again(ctx, turn, Stage::Wake, &wake_errand(turn.tools_allowed, ctx.chat_id)).await?
    {
      turn.reply = again.clone();
    }

    if turn.tools_allowed
      && promises_after(&turn.reply.text)
      && !wakes(ctx.receiver, ctx.agent, &turn.spoken).await
      && let Some(last) = ask_again(ctx, turn, Stage::Wake, &wake_errand(false, ctx.chat_id)).await?
    {
      turn.reply = last.clone();
    }
  }

  Ok(())
}

/// The owner setting what the agent does from a situation on stands only where this turn filed a rule: read off
/// his own words in the turn, asked once and no more, and never of a turn whose tools are off.
pub async fn rule(ctx: &Ctx<'_>, turn: &mut Turn) -> Result<(), Failure> {
  let owner = ctx.receiver.deployment().owner_id;
  let owner_set_one = turn
    .batch
    .iter()
    .any(|message| message.sender_id == Some(owner) && sets_rule_later(&message.text));

  if turn.tools_allowed
    && owner_set_one
    && !recorded(ctx.receiver, ctx.agent, &turn.spoken, Some("rule")).await
  {
    let errand = fill_marks(&SECOND_ASKS.errands.rule, &[("chat", &ctx.chat_id.to_string())]);

    if let Some(again) = ask_again(ctx, turn, Stage::Rule, &errand).await? {
      turn.reply = again.clone();
    }
  }

  Ok(())
}

/// An answer is held to the words the owner banned: one using a term outside its quoted spans is written once
/// more with the terms named, and the second writing stands where it is in the room's language.
pub async fn wording(ctx: &Ctx<'_>, turn: &mut Turn) -> Result<(), Failure> {
  if is_skip(&turn.reply.text) {
    return Ok(());
  }

  let terms = banned_terms_in(&turn.reply.text, &glossary(ctx.receiver));

  if !terms.is_empty() {
    let errand = fill_marks(&SECOND_ASKS.errands.wording, &[("terms", &banned_terms_line(&terms))]);
    let again = ask_again(ctx, turn, Stage::Wording, &errand).await?;

    if let Some(again) = again.filter(|again| in_room_language(&again.text, &turn.recent)) {
      turn.reply = again.clone();
    }
  }

  Ok(())
}

/// The banned words of the deployment's wording document, or none where it is not named or cannot be read.
fn glossary(receiver: &Receiver) -> Vec<BannedTerm> {
  receiver
    .deployment()
    .wording_document
    .as_ref()
    .map(|document| read_glossary(document.as_path()))
    .unwrap_or_default()
}

/// The checks, as the conversations hold them.
#[derive(Clone)]
pub struct Checker(Receiver);

impl Checker {
  pub fn new(receiver: Receiver) -> Self {
    Checker(receiver)
  }
}

impl crate::chat::conversation::Checks for Checker {
  fn checks<'a>(
    &'a self,
    conversations: &'a Conversations,
    _receiver: &'a Receiver,
    agent: &'a str,
    held: &'a Held,
    checked: &'a Checked<'a>,
  ) -> BoxFuture<'a, Result<Reply, Failure>> {
    Box::pin(async move {
      let asker = Asker {
        room: &checked.room,
        via: &checked.via,
        privileged: checked.privileged,
        tools_allowed: checked.tools_allowed,
        selection: checked.selection,
        pending: &checked.pending,
      };
      let ctx = Ctx {
        conversations,
        receiver: &self.0,
        agent,
        chat_id: held.chat_id,
        held,
        asker: &asker,
      };
      let mut turn = Turn {
        reply: checked.reply.clone(),
        spoken: checked.reply.run.clone().map(|run| vec![run.to_owned()]).unwrap_or_default(),
        scope: checked.scope.clone(),
        recent: checked.recent.iter().map(|message| message.text.clone()).collect(),
        tools_allowed: checked.tools_allowed,
        batch: checked
          .batch
          .iter()
          .map(|message| Said {
            sender_id: message.sender_id,
            text: message.text.clone(),
          })
          .collect(),
      };

      owed(&ctx, &mut turn, checked.owed).await?;
      language(&ctx, &mut turn).await?;
      record(&ctx, &mut turn).await?;
      wake(&ctx, &mut turn).await?;
      rule(&ctx, &mut turn).await?;
      wording(&ctx, &mut turn).await?;

      Ok(turn.reply.clone())
    })
  }
}

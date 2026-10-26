//! The draft process (docs/drafting.md): a round's message is made by asking its session the questions its row of
//! the table names, one turn each, the first riding the round's own turn; every answer before the last is notes,
//! and the last answer, or one that opens on the send marker, is the message. Then come the items the code's
//! readings call for and the checks on the final text, each asked once as one more turn. Every turn is written in
//! the round log.

pub mod checks;
pub mod log;
pub mod table;

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use receiver_codes::FixedLine;

use receiver_policy::drafting::DRAFTING;
use receiver_policy::fill_marks;
use receiver_policy::second_asks::SECOND_ASKS;
use serde_json::to_string as stringify;

use super::ask::Carry;
use super::conversation::{
  Asking, BoxFuture, Carrying, Conversations, Drafted, Drafting, Failure, Follower, Held, Made, Reply,
};
use super::languages;
use super::mid_turn::{Push, paragraph};
use super::promises::{promises_after, promises_later, sets_rule_later};
use super::prompt::{Scope, scope_line};
use super::records::run_records;
use super::sentinels::{Mark, SKIP, is_skip, is_wait, round_marks, sent_now, without_round_marks};
use super::turnover::note_session;
use crate::form::strip_prompt_scaffolding;
use crate::now_ms;
use crate::receiver::{Inbound, Receiver};

use checks::{Check, Reading, failing, has_skip, light, own_lines};
use log::{Answered, Ended, Kind, Log};
use table::{PastCeiling, resolve, rows};

/// What an item or a check goes out with beside its errand: the turn's scope, the messages waiting for the next
/// turn, and that they are not acted on now.
pub fn words(errand: &str, scope: &Scope, pending: &[i64]) -> String {
  format!(
    "{errand}\n{}\n{}{}\n{}",
    scope_line(scope),
    SECOND_ASKS.next_turn,
    stringify(&pending).unwrap_or_default(),
    SECOND_ASKS.later,
  )
}

/// The process a round opens before its turn goes out (docs/drafting.md, 「The process table」): its questions by
/// name with their texts, the place of the question a send is read from (from 0), the row of the table it read,
/// its ceiling and what is done past it, the bound of one turn, and whether the round's room has heard it is still
/// at work. A round no row matches asks nothing and has neither ceiling nor bound.
#[derive(Clone, Debug)]
pub struct Plan {
  pub questions: Vec<(String, String)>,
  pub send_from_at: usize,
  pub row: String,
  pub ceiling_ms: Option<i64>,
  pub past_ceiling: PastCeiling,
  pub turn_ms: Option<u64>,
  pub told: Arc<AtomicBool>,
}

impl Default for Plan {
  fn default() -> Self {
    Plan {
      questions: vec![],
      send_from_at: 0,
      row: String::new(),
      ceiling_ms: None,
      past_ceiling: PastCeiling::Send,
      turn_ms: None,
      told: Arc::default(),
    }
  }
}

/// One turn of the process carried to its end; where it runs past the plan's bound, the round's room is told once
/// in the round, in the fixed line, that the agent is still at work, and the turn goes on (docs/drafting.md,
/// 「Time」).
pub async fn bounded<T>(
  receiver: &Receiver,
  chat_id: i64,
  plan: &Plan,
  turn: impl Future<Output = T>,
) -> T {
  let Some(bound) = plan.turn_ms.filter(|_| !plan.told.load(Ordering::SeqCst)) else {
    return turn.await;
  };
  let mut turn = std::pin::pin!(turn);

  tokio::select! {
    done = &mut turn => return done,
    _ = tokio::time::sleep(Duration::from_millis(bound)) => {}
  }

  if !plan.told.swap(true, Ordering::SeqCst) {
    let line = receiver.speech().rendered(&FixedLine::StillWorking).await;

    if let Err(why) = receiver.speech().say(chat_id, &line, true).await {
      eprintln!("chat {chat_id}: the line that the turn is still at work was not said: {why}");
    }
  }

  turn.await
}

impl Plan {
  /// Whether the round's turn carries a question that is not the process's last, so that it asks no message.
  pub fn questioned(&self) -> bool {
    self.questions.len() > 1
  }

  /// Whether a send is read in the answer to the question at a place (from 0).
  pub fn send_open(&self, at: usize) -> bool {
    at >= self.send_from_at
  }

  /// What a question's turn says of the send marker: that a send is read at this question, or from which
  /// question on it is.
  pub fn send_line(&self, at: usize) -> String {
    if self.send_open(at) {
      return DRAFTING.send_open.clone();
    }

    let name = self.questions.get(self.send_from_at).map(|(name, _)| name.as_str()).unwrap_or_default();

    fill_marks(&DRAFTING.send_closed, &[("from", &(self.send_from_at + 1).to_string()), ("name", name)])
  }

  /// What the first question adds to the round's turn, where the process asks any: the opening, or the last
  /// question's line where it is also the last, then the question. The turn's own scope stands in it already.
  pub fn first_words(&self) -> Option<String> {
    let (name, text) = self.questions.first()?;
    let count = self.questions.len();
    let lead = if count == 1 { &DRAFTING.last_lead } else { &DRAFTING.opening };
    let send = self.send_line(0);
    let lead =
      fill_marks(lead, &[("number", "1"), ("count", &count.to_string()), ("name", name), ("send", &send)]);

    Some(format!("{lead}\n\n{text}"))
  }
}

/// An answer as the process reads it: a skip standing anywhere in it outside code, which wins; a send opening it,
/// the words after the marker being the message, and a send with nothing after it a skip; or an answer.
pub fn read_answer(text: &str) -> (String, Answered) {
  if has_skip(text) || is_skip(text) {
    return (text.to_owned(), Answered::Skip);
  }

  match sent_now(text) {
    Some("") => (SKIP.to_owned(), Answered::Skip),
    Some(words) => (words.to_owned(), Answered::Send),
    None => (text.to_owned(), Answered::Answered),
  }
}

/// The opening of a later question turn: the lead naming its place and what it says of the send marker, then
/// the question and the turn's scope.
pub fn question_words(
  name: &str,
  text: &str,
  number: usize,
  count: usize,
  send: &str,
  scope: &Scope,
) -> String {
  let lead = if number == count { &DRAFTING.last_lead } else { &DRAFTING.lead };
  let lead = fill_marks(
    lead,
    &[
      ("number", &number.to_string()),
      ("count", &count.to_string()),
      ("name", name),
      ("send", send),
    ],
  );

  format!("{lead}\n\n{text}\n{}", scope_line(scope))
}

/// The line every question but the last closes on: the messages still waiting, shown with the next question.
pub fn waiting_line(pending: &[i64]) -> String {
  fill_marks(&DRAFTING.waiting, &[("ids", &stringify(&pending).unwrap_or_default())])
}

/// The message once the closing check answered the words that arrived: the standing message kept and the answer
/// added after it, the two one message; an answer that holds the standing message whole is the message as written.
pub fn appended(standing: &str, answer: &str) -> String {
  let (standing, answer) = (standing.trim(), answer.trim());

  if standing.is_empty() || answer.contains(standing) {
    return answer.to_owned();
  }

  format!("{standing}\n\n{answer}")
}

/// An answer that says something: not empty, not a skip and not a wait.
pub fn usable(text: &str) -> bool {
  !text.is_empty() && !is_skip(text) && !is_wait(text)
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

// Whether the process's runs wrote something down, of one kind where one is named; a watch is not among what any
// kind means, since it holds nothing and only wakes. One that cannot be read reads as nothing written.
async fn recorded(receiver: &Receiver, agent: &str, runs: &[String], kind: Option<&str>) -> bool {
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

// Whether the process's runs set something that wakes the agent.
async fn wakes(receiver: &Receiver, agent: &str, runs: &[String]) -> bool {
  match run_records(receiver, agent, runs).await {
    Ok(records) => records.iter().any(|record| record.wakes),
    Err(why) => {
      eprintln!("the turn's records could not be read: {}", why.detail);

      false
    }
  }
}

/// The process as the conversations hold it: the receiver it runs on.
#[derive(Clone)]
pub struct Drafter {
  receiver: Receiver,
}

impl Drafter {
  pub fn new(receiver: Receiver) -> Self {
    Drafter { receiver }
  }
}

// One round's process as it runs: where it stands, the runs its turns opened, the texts of the questions it
// asked, the reply the last turn carried and the log; the messages that arrived meanwhile and were shown to it,
// what the round holds by path, the marks its answers set and whether the last answer was nothing but marks.
struct Run<'a> {
  drafter: &'a Drafter,
  conversations: &'a Conversations,
  agent: &'a str,
  held: &'a Held,
  drafted: &'a Drafted<'a>,
  spoken: Vec<String>,
  asked: Vec<String>,
  latest: Reply,
  log: Log,
  shown: Vec<Inbound>,
  holds: HashSet<String>,
  marks: Vec<(Mark, i64)>,
  only_marks: bool,
}

impl Run<'_> {
  // One more turn of the same session: its answer where it gave one, as the process reads it with its marks taken
  // out, none where its run failed, the stop raised. No room's words are pushed into it while it works.
  async fn ask(
    &mut self,
    kind: Kind,
    name: &str,
    words: &str,
    send_open: bool,
  ) -> Result<Option<(Reply, Answered)>, Failure> {
    let drafted = self.drafted;
    let chat = self.held.chat_id;
    let started = now_ms();
    let asking = Asking {
      chat_id: chat,
      session: self.held.session.clone().unwrap_or_default(),
      // A later turn of the process carries no reference of its own: the index went in with the round's turn.
      prompt: words.to_owned(),
      via: match drafted.via.as_str() {
        "" => None,
        road => Some(road.to_owned()),
      },
      privileged: drafted.privileged,
      answers: vec![],
      began: started,
      context: drafted.selection.context.clone(),
    };
    let held = Held {
      agent: self.agent.to_owned(),
      chat_id: chat,
      session: self.latest.session.clone().or(self.held.session.clone()),
      account: self.held.account.clone(),
      context_tokens: self.latest.context_tokens.or(self.held.context_tokens),
      all_from_owner: drafted.privileged,
      replaced: vec![],
      last_consumed_id: self.held.last_consumed_id,
    };
    let follower = Follower::new(self.conversations, chat);
    let receiver = &self.drafter.receiver;
    let carrying = Carry::new(receiver.clone());
    let carried = bounded(receiver, chat, drafted.plan, carrying.carry(&asking, &held, &follower)).await;

    self.only_marks = false;

    match carried {
      Ok(answered) => {
        let text = self.marked(&strip_prompt_scaffolding(&answered.text));
        // An answer of nothing but marks says nothing more: no skip, and no words.
        let (text, ending) = match (self.only_marks, text.is_empty()) {
          (true, _) => (text, Answered::Answered),
          (false, true) => read_answer(SKIP),
          (false, false) => read_answer(&text),
        };
        // A send before the question a send is read from is notes like any answer.
        let ending = if ending == Answered::Send && !send_open { Answered::Answered } else { ending };
        let answer = Reply { text, ..answered };

        if let Err(why) = note_session(
          &self.drafter.receiver,
          self.agent,
          chat,
          drafted.privileged,
          answer.session.as_deref(),
          drafted.tools_allowed,
        )
        .await
        {
          eprintln!("chat {chat}: the session's launch: {}", why.detail);
        }

        self.spoken.extend(answer.run.clone());
        self.log.turn(kind, name, Some(&answer.text), ending, now_ms() - started).await;
        self.latest = answer.clone();

        Ok(Some((answer, ending)))
      }
      Err(failure) => {
        self.log.turn(kind, name, None, Answered::Failed, now_ms() - started).await;

        if matches!(failure, Failure::Stopped) {
          return Err(failure);
        }

        eprintln!("chat {chat}: draft {name} failed: {failure:?}");

        Ok(None)
      }
    }
  }
}

impl Run<'_> {
  // An answer with the marks it sets on the messages shown to the round kept and taken out of it.
  fn marked(&mut self, text: &str) -> String {
    let marks = round_marks(text);
    let bare = without_round_marks(text);

    self.only_marks = !marks.is_empty() && bare.is_empty();
    self.marks.extend(marks);

    bare
  }

  // The room's words that waited since the last look, built into the block the turn about to go out carries
  // (docs/drafting.md, 「What came in meanwhile」), with the words it shows; none where nothing waited, or where
  // the block could not be built and the words went back to wait.
  async fn arrived(&mut self) -> Option<(String, Vec<Inbound>)> {
    let drafted = self.drafted;
    let chat = self.held.chat_id;
    let receiver = &self.drafter.receiver;
    let took = self.conversations.meanwhile(chat, drafted.privileged);

    if took.is_empty() {
      return None;
    }

    let built = paragraph(&Push {
      receiver,
      agent: self.agent,
      owner: receiver.owner_id(),
      chat_id: chat,
      room: drafted.room,
      round: drafted.round,
      messages: &took,
      instructions: &[],
      events: &[],
      holds: &self.holds,
      consumed: self.held.last_consumed_id,
      tools_allowed: drafted.tools_allowed,
      reader: Some(receiver.roads()),
    })
    .await;

    match built {
      Ok(built) => {
        self.holds.extend(built.reference);

        Some((built.text, took))
      }
      Err(why) => {
        eprintln!("chat {chat}: the words that came meanwhile could not be put in: {}", why.detail);
        self.conversations.give_back_words(chat, took);

        None
      }
    }
  }

  // Every message the process showed back to wait, as a stop cuts it off.
  fn give_back_shown(&mut self) {
    self.conversations.give_back_words(self.held.chat_id, std::mem::take(&mut self.shown));
  }

  // The messages shown to the round, as its answers marked them: those its message answers, those that need no
  // answer and those answered in a round of their own. A message marked both ways is separate.
  fn sorted(&self) -> (Vec<Inbound>, Vec<Inbound>, Vec<Inbound>) {
    let marked = |mark: Mark, id: i64| self.marks.contains(&(mark, id));
    let mut sorted = (vec![], vec![], vec![]);

    for message in &self.shown {
      let id = message.message_id;

      if marked(Mark::Separate, id) {
        sorted.2.push(message.clone());
      } else if marked(Mark::Done, id) {
        sorted.1.push(message.clone());
      } else {
        sorted.0.push(message.clone());
      }
    }

    sorted
  }

  // What the process hands back: the text with what the last turn carried of the session, and the messages it was
  // shown as their marks sort them.
  fn made(&self, text: &str) -> Made {
    let (took, done, separate) = self.sorted();

    Made {
      reply: Reply {
        text: text.to_owned(),
        ..self.latest.clone()
      },
      took,
      done,
      separate,
    }
  }
}

/// What the process asks after its last question, in their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
  Record,
  Wake,
  Rule,
}

impl Item {
  fn word(self) -> &'static str {
    match self {
      Self::Record => "record",
      Self::Wake => "wake",
      Self::Rule => "rule",
    }
  }
}

impl Drafter {
  // The process a round opens: the row its session's provider and tier and its agent match, and the questions
  // that row asks of a turn that took what this one took.
  async fn plan(&self, agent: &str, chat: i64, batch: &[Inbound], instructions: usize) -> Plan {
    let receiver = &self.receiver;
    let (provider, tier) = Carry::new(receiver.clone()).standing(agent, chat).await;
    let table = rows(receiver.db()).await.unwrap_or_else(|why| {
      eprintln!("chat {chat}: the draft process table could not be read: {}", why.detail);
      vec![]
    });
    let Some(process) = resolve(&table, provider.as_deref(), tier.as_deref(), agent) else {
      return Plan {
        row: "none".to_owned(),
        ..Plan::default()
      };
    };
    let is_light = light(batch, instructions, process.light_chars);
    let list = if is_light { &process.light } else { &process.rounds };
    // A light turn reads a send from its first question whatever the row names.
    let send_from_at = if is_light {
      0
    } else {
      list.iter().position(|name| name == &process.send_from).unwrap_or(0)
    };
    let catalogue = table::questions(receiver.db()).await.unwrap_or_default();
    let questions = list
      .iter()
      .map(|name| {
        let text = catalogue
          .iter()
          .find(|question| &question.name == name)
          .map(|question| question.text.clone())
          .unwrap_or_default();

        (name.clone(), text)
      })
      .collect();

    Plan {
      questions,
      send_from_at,
      row: format!("{}/{}/{}", process.provider, process.tier, process.agent),
      ceiling_ms: Some(i64::from(process.ceiling_seconds) * 1000),
      past_ceiling: process.past_ceiling,
      turn_ms: Some(u64::from(process.turn_seconds) * 1000),
      told: Arc::default(),
    }
  }

  async fn run(
    &self,
    conversations: &Conversations,
    agent: &str,
    held: &Held,
    drafted: &Drafted<'_>,
  ) -> Result<Made, Failure> {
    let receiver = &self.receiver;
    let chat = held.chat_id;
    let owner = receiver.owner_id();
    let plan = drafted.plan;
    let row = &plan.row;
    let count = plan.questions.len();
    let mut run = Run {
      drafter: self,
      conversations,
      agent,
      held,
      drafted,
      spoken: drafted.reply.run.clone().into_iter().collect(),
      asked: plan.questions.first().map(|(_, text)| text.clone()).into_iter().collect(),
      latest: drafted.reply.clone(),
      log: Log::new(conversations, drafted.round, chat, agent),
      shown: vec![],
      holds: drafted.selection.entries.iter().map(|entry| entry.path.clone()).collect(),
      marks: vec![],
      only_marks: false,
    };
    let first_text = run.marked(&drafted.reply.text);
    let (text, first_read) = read_answer(if first_text.is_empty() { SKIP } else { &first_text });
    // A send before the question a send is read from is notes like any answer.
    let first_read = if first_read == Answered::Send && !plan.send_open(0) {
      Answered::Answered
    } else {
      first_read
    };
    let first = Reply {
      text,
      ..drafted.reply.clone()
    };
    let (kind, name) = match plan.questions.first() {
      Some((name, _)) => (Kind::Question, name.as_str()),
      None => (Kind::Turn, "turn"),
    };

    run.latest = first.clone();
    run.log.turn(kind, name, Some(&first.text), first_read, now_ms() - drafted.began).await;

    // The ceiling counts from the round's turn, which carried the first question.
    let ceiling_at = drafted.opened + plan.ceiling_ms.unwrap_or(i64::MAX / 2);
    let past = |ceiling_at: i64| now_ms() >= ceiling_at;
    let mut message: Option<Reply> = None;
    let mut skipped = first_read == Answered::Skip;
    let mut ceiling = false;

    // The first answer is the message where it sent one, or where the first question was the last or the turn
    // asked none; any other is notes.
    if !skipped && (first_read == Answered::Send || count <= 1) {
      message = Some(first);
    }

    let mut at = 1;

    while message.is_none() && !skipped && at < count {
      // Past the ceiling no message stands before `final`: it is asked at once, or nothing goes out.
      if !ceiling && past(ceiling_at) {
        ceiling = true;

        if plan.past_ceiling == PastCeiling::Skip {
          skipped = true;
          break;
        }

        at = count - 1;
      }

      let number = at + 1;
      let last = number == count;
      let (name, text) = &plan.questions[at];
      let send = plan.send_line(at);
      let mut words = question_words(name, text, number, count, &send, drafted.scope);

      run.asked.push(text.clone());

      // The room's words that waited since the last answer stand under this question.
      let block = run.arrived().await;

      if let Some((text, _)) = &block {
        words = format!("{words}\n\n{text}");
      }

      if !last {
        words = format!("{words}\n{}", waiting_line(&conversations.pending(chat)));
      }

      run.log.start(number, count, name);

      let asked = run.ask(Kind::Question, name, &words, plan.send_open(at)).await;
      let block = block.map(|(_, took)| took).unwrap_or_default();

      match asked {
        Ok(Some((answer, read))) => {
          run.shown.extend(block);

          // A wait is no message: the last question answering one sends nothing.
          if read == Answered::Skip || (last && is_wait(&answer.text)) {
            skipped = true;
            break;
          }

          if last || read == Answered::Send {
            message = Some(answer);
          }
        }
        Ok(None) => {
          conversations.give_back_words(chat, block);

          if last {
            break;
          }

          // A question whose run failed is passed over to the last.
          at = count - 1;
          continue;
        }
        Err(failure) => {
          conversations.give_back_words(chat, block);
          run.give_back_shown();

          return Err(failure);
        }
      }

      at += 1;
    }

    if skipped {
      message = None;
    }

    // Once the message stands, the room's words that waited since the last answer are shown in one closing check.
    if let Some(standing) = &message
      && !past(ceiling_at)
      && let Some((block, took)) = run.arrived().await
    {
      let words = format!("{}\n{}\n\n{block}", DRAFTING.arrived, scope_line(drafted.scope));

      match run.ask(Kind::Item, "arrived", &words, true).await {
        Ok(Some((answer, read))) => {
          run.shown.extend(took);

          if read == Answered::Skip {
            message = None;
            skipped = true;
          } else if is_wait(&answer.text) || run.only_marks || answer.text.trim().is_empty() {
            message = Some(standing.clone());
          } else {
            let text = appended(&standing.text, &answer.text);

            message = Some(Reply { text, ..answer });
          }
        }
        Ok(None) => conversations.give_back_words(chat, took),
        Err(failure) => {
          conversations.give_back_words(chat, took);
          run.give_back_shown();

          return Err(failure);
        }
      }
    }

    let (answered, _, _) = run.sorted();
    let owed = drafted.owed
      + if chat == owner && !skipped {
        answered.iter().filter(|message| message.sender_id == Some(owner)).count()
      } else {
        0
      };

    // The owner's own chat is always answered: a process that ended with no message over the owner's words is
    // asked once more, whatever the time.
    if message.is_none() && owed > 0 {
      let errand = fill_marks(&SECOND_ASKS.errands.owed, &[("count", &owed.to_string())]);

      match run
        .ask(Kind::Item, "owed", &words(&errand, drafted.scope, &drafted.pending), true)
        .await
      {
        Ok(Some((answer, read))) if usable(&answer.text) && read != Answered::Skip => {
          message = Some(answer)
        }
        Ok(_) => {}
        Err(failure) => {
          run.give_back_shown();

          return Err(failure);
        }
      }
    }

    let Some(mut message) = message else {
      let ended = if ceiling { Ended::CeilingSkip } else { Ended::Skip };

      run.log.end(ended, row, now_ms() - drafted.began).await;

      return Ok(run.made(SKIP));
    };

    // The items the code's readings call for.
    for item in [Item::Record, Item::Wake, Item::Rule] {
      if past(ceiling_at) {
        break;
      }

      let errand = match item {
        Item::Record => (promises_later(&message.text)
          && !recorded(receiver, agent, &run.spoken, None).await)
          .then(|| record_errand(drafted.tools_allowed)),
        Item::Wake => (promises_after(&message.text) && !wakes(receiver, agent, &run.spoken).await)
          .then(|| wake_errand(drafted.tools_allowed, chat)),
        Item::Rule => {
          let set_one = drafted
            .batch
            .iter()
            .chain(answered.iter())
            .any(|said| said.sender_id == Some(owner) && sets_rule_later(&said.text));

          (drafted.tools_allowed && set_one && !recorded(receiver, agent, &run.spoken, Some("rule")).await)
            .then(|| fill_marks(&SECOND_ASKS.errands.rule, &[("chat", &chat.to_string())]))
        }
      };
      let Some(errand) = errand else { continue };

      let asked = run
        .ask(Kind::Item, item.word(), &words(&errand, drafted.scope, &drafted.pending), true)
        .await;

      match asked {
        Ok(Some((answer, read))) => {
          if read == Answered::Skip {
            run.log.end(Ended::Skip, row, now_ms() - drafted.began).await;

            return Ok(run.made(SKIP));
          }

          if usable(&answer.text) {
            message = answer;
          }
        }
        Ok(None) => {}
        Err(failure) => {
          run.give_back_shown();

          return Err(failure);
        }
      }
    }

    // The checks on the final text, each asked once. The answer is held to the languages of the people it
    // answers, each person's own, never to one language of the turn's (docs/answer-checks.md).
    let mut spoken: Vec<Inbound> = drafted.batch.to_vec();

    for said in answered.iter() {
      if !spoken.iter().any(|word| word.message_id == said.message_id) {
        spoken.push(said.clone());
      }
    }

    let addressees =
      match languages::addressees(receiver, chat, owner, &spoken, drafted.recent, drafted.room.identity)
        .await
      {
        Ok(addressees) => addressees,
        Err(why) => {
          eprintln!("chat {chat}: the people's languages were not read: {why}");
          vec![]
        }
      };
    let held_to = languages::held_to(receiver, &addressees);
    let people = languages::clauses(&addressees);
    let mut asked: BTreeSet<Check> = BTreeSet::new();
    // The text the language check was asked over, which goes out as it stood where the second answer still
    // holds none of the people's languages; the check never holds an answer back.
    let mut before_language: Option<Reply> = None;
    let mut language_read = true;

    loop {
      let lines = own_lines(&run.asked);
      let reading = Reading {
        languages: &held_to,
        people: &people,
        language: language_read,
        own_lines: &lines,
        max_chars: DRAFTING.max_chars,
      };
      let failed = failing(&message.text, &reading);

      if failed.is_empty() {
        break;
      }

      if failed.contains(&Check::Language) && asked.contains(&Check::Language) {
        eprintln!(
          "chat {chat}: the second answer still held none of the people's languages; the first goes out"
        );

        if let Some(first) = before_language.take() {
          message = first;
        }

        language_read = false;
        continue;
      }

      let blocking: Vec<Check> = failed.iter().copied().filter(|check| *check != Check::Language).collect();

      if past(ceiling_at) && blocking.is_empty() {
        break;
      }

      if past(ceiling_at) || blocking.iter().any(|check| asked.contains(check)) {
        let names: Vec<&str> = blocking.iter().map(|check| check.word()).collect();

        eprintln!("chat {chat}: the message was withheld: {} failed", names.join(", "));
        run
          .log
          .end(Ended::Withheld, &format!("{row}; {}", names.join(", ")), now_ms() - drafted.began)
          .await;

        return Ok(run.made(SKIP));
      }

      for check in failed {
        let errand = checks::errand(check, &message.text, &reading);

        asked.insert(check);

        if check == Check::Language {
          before_language = Some(message.clone());
        }

        let answered = run
          .ask(Kind::Check, check.word(), &words(&errand, drafted.scope, &drafted.pending), true)
          .await;

        match answered {
          Ok(Some((answer, read))) => {
            if read == Answered::Skip {
              run.log.end(Ended::Skip, row, now_ms() - drafted.began).await;

              return Ok(run.made(SKIP));
            }

            if !is_wait(&answer.text) {
              message = answer;
            }
          }
          Ok(None) => {}
          Err(failure) => {
            run.give_back_shown();

            return Err(failure);
          }
        }
      }
    }

    let ended = if ceiling { Ended::CeilingMessage } else { Ended::Message };

    run.log.end(ended, row, now_ms() - drafted.began).await;

    Ok(run.made(&message.text))
  }
}

impl Drafting for Drafter {
  fn plan<'a>(
    &'a self,
    agent: &'a str,
    chat_id: i64,
    batch: &'a [Inbound],
    instructions: usize,
  ) -> BoxFuture<'a, Plan> {
    Box::pin(Drafter::plan(self, agent, chat_id, batch, instructions))
  }

  fn draft<'a>(
    &'a self,
    conversations: &'a Conversations,
    agent: &'a str,
    held: &'a Held,
    drafted: &'a Drafted<'a>,
  ) -> BoxFuture<'a, Result<Made, Failure>> {
    Box::pin(self.run(conversations, agent, held, drafted))
  }
}

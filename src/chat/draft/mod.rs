//! The draft process (docs/drafting.md): a round's message is made by asking its session the questions its row of
//! the table names, one turn each, the first riding the round's own turn; every answer before the last is notes,
//! and the last answer, or one that opens on the send marker, is the message. Then come the items the code's
//! readings call for and the checks on the final text, each asked once as one more turn. Every turn is written in
//! the round log.

pub mod checks;
pub mod log;
pub mod table;

use std::collections::{BTreeSet, HashSet};
use std::time::Duration;

use receiver_policy::drafting::DRAFTING;
use receiver_policy::fill_marks;
use receiver_policy::second_asks::SECOND_ASKS;
use serde_json::to_string as stringify;

use super::ask::Carry;
use super::conversation::{
  Asking, BoxFuture, Carrying, Conversations, Drafted, Drafting, Failure, Follower, Held, Made, Reply,
};
use super::mid_turn::{Push, paragraph};
use super::promises::{promises_after, promises_later, sets_rule_later};
use super::prompt::{Scope, scope_line};
use super::records::run_records;
use super::sentinels::{SKIP, is_skip, is_wait, sent_now};
use super::turnover::note_session;
use super::window::composed;
use crate::form::strip_prompt_scaffolding;
use crate::now_ms;
use crate::receiver::{Inbound, Receiver};
use crate::wording::{BannedTerm, read_glossary};

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
/// name with their texts, the row of the table it read, its ceiling and what is done past it. A round no row
/// matches asks nothing and has no ceiling.
#[derive(Clone, Debug)]
pub struct Plan {
  pub questions: Vec<(String, String)>,
  pub row: String,
  pub ceiling_ms: Option<i64>,
  pub past_ceiling: PastCeiling,
}

impl Default for Plan {
  fn default() -> Self {
    Plan {
      questions: vec![],
      row: String::new(),
      ceiling_ms: None,
      past_ceiling: PastCeiling::Send,
    }
  }
}

impl Plan {
  /// Whether the round's turn carries a question that is not the process's last, so that it asks no message.
  pub fn questioned(&self) -> bool {
    self.questions.len() > 1
  }

  /// What the first question adds to the round's turn, where the process asks any: the opening, or the last
  /// question's line where it is also the last, then the question. The turn's own scope stands in it already.
  pub fn first_words(&self) -> Option<String> {
    let (name, text) = self.questions.first()?;
    let count = self.questions.len();
    let lead = if count == 1 { &DRAFTING.last_lead } else { &DRAFTING.opening };
    let lead = fill_marks(lead, &[("number", "1"), ("count", &count.to_string()), ("name", name)]);

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

/// The opening of a later question turn: the lead naming its place, then the question and the turn's scope.
pub fn question_words(name: &str, text: &str, number: usize, count: usize, scope: &Scope) -> String {
  let lead = if number == count { &DRAFTING.last_lead } else { &DRAFTING.lead };
  let lead =
    fill_marks(lead, &[("number", &number.to_string()), ("count", &count.to_string()), ("name", name)]);

  format!("{lead}\n\n{text}\n{}", scope_line(scope))
}

/// The line every question but the last closes on: the messages waiting, shown with the last question.
pub fn waiting_line(pending: &[i64]) -> String {
  fill_marks(&DRAFTING.waiting, &[("ids", &stringify(&pending).unwrap_or_default())])
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

/// The banned words of the deployment's wording document, or none where it is not named or cannot be read.
fn glossary(receiver: &Receiver) -> Vec<BannedTerm> {
  receiver
    .deployment()
    .wording_document
    .as_ref()
    .map(|document| read_glossary(document.as_path()))
    .unwrap_or_default()
}

/// The process as the conversations hold it: the receiver it runs on and how often the last question's working
/// turn is looked at for words that came for its room.
#[derive(Clone)]
pub struct Drafter {
  receiver: Receiver,
  look: Duration,
}

impl Drafter {
  pub fn new(receiver: Receiver, look: Duration) -> Self {
    Drafter { receiver, look }
  }
}

// One round's process as it runs: where it stands, the runs its turns opened, the texts of the questions it
// asked, the reply the last turn carried and the log.
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
  // What the last question grew by while it worked: the words pushed into it past those it began with, and the
  // rows they settled.
  grown: (Vec<Inbound>, Vec<i64>),
}

impl Run<'_> {
  // One more turn of the same session: its answer where it gave one, as the process reads it, none where its run
  // failed, the stop raised. The last question names how many words it carries from the start, which pushes grow
  // it past.
  async fn ask(
    &mut self,
    kind: Kind,
    name: &str,
    words: &str,
    last: Option<usize>,
  ) -> Result<Option<(Reply, Answered)>, Failure> {
    let drafted = self.drafted;
    let chat = self.held.chat_id;
    let started = now_ms();
    let asking = Asking {
      chat_id: chat,
      session: self.held.session.clone().unwrap_or_default(),
      prompt: composed(drafted.selection, words),
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
    let carrying = if last.is_some() {
      Carry::looking_every(self.drafter.receiver.clone(), self.drafter.look)
    } else {
      Carry::new(self.drafter.receiver.clone())
    };
    let carried = carrying.carry(&asking, &held, &follower).await;

    if let Some(from) = last {
      self.grown = Conversations::close_last(&follower, from);
    }

    match carried {
      Ok(answered) => {
        let text = strip_prompt_scaffolding(&answered.text);
        let (text, ending) = read_answer(if text.is_empty() { SKIP } else { &text });
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
    let list = if light(batch, instructions, process.light_chars) {
      &process.light
    } else {
      &process.rounds
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
      row: format!("{}/{}/{}", process.provider, process.tier, process.agent),
      ceiling_ms: Some(i64::from(process.ceiling_seconds) * 1000),
      past_ceiling: process.past_ceiling,
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
    let (text, first_read) = read_answer(&drafted.reply.text);
    let first = Reply {
      text,
      ..drafted.reply.clone()
    };
    let mut run = Run {
      drafter: self,
      conversations,
      agent,
      held,
      drafted,
      spoken: first.run.clone().into_iter().collect(),
      asked: plan.questions.first().map(|(_, text)| text.clone()).into_iter().collect(),
      latest: first.clone(),
      log: Log::new(receiver, drafted.round, chat, agent),
      grown: (vec![], vec![]),
    };
    let (kind, name) = match plan.questions.first() {
      Some((name, _)) => (Kind::Question, name.as_str()),
      None => (Kind::Turn, "turn"),
    };

    run.log.turn(kind, name, Some(&first.text), first_read, now_ms() - drafted.began).await;

    // The ceiling counts from the round's turn, which carried the first question.
    let ceiling_at = drafted.opened + plan.ceiling_ms.unwrap_or(i64::MAX / 2);
    let mut message: Option<Reply> = None;
    let mut skipped = first_read == Answered::Skip;
    let mut ceiling = false;
    let mut took: Vec<Inbound> = vec![];
    let mut settling: Vec<i64> = vec![];

    // The first answer is the message where it sent one, or where the first question was the last or the turn
    // asked none; any other is notes.
    if !skipped && (first_read == Answered::Send || count <= 1) {
      message = Some(first);
    }

    let mut at = 1;

    while message.is_none() && !skipped && at < count {
      // Past the ceiling no message stands before `final`: it is asked at once, or nothing goes out.
      if !ceiling && now_ms() >= ceiling_at {
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
      let mut words = question_words(name, text, number, count, drafted.scope);

      run.asked.push(text.clone());

      if last {
        took = conversations.meanwhile(chat, drafted.privileged);

        if !took.is_empty() {
          let holds: HashSet<String> =
            drafted.selection.entries.iter().map(|entry| entry.path.clone()).collect();
          let built = paragraph(&Push {
            receiver,
            agent,
            owner,
            chat_id: chat,
            room: drafted.room,
            round: drafted.round,
            messages: &took,
            instructions: &[],
            events: &[],
            holds: &holds,
            consumed: held.last_consumed_id,
            tools_allowed: drafted.tools_allowed,
            reader: Some(receiver.roads()),
          })
          .await;

          match built {
            Ok(built) => words = format!("{words}\n\n{}", built.text),
            Err(why) => {
              eprintln!("chat {chat}: the words that came meanwhile could not be put in: {}", why.detail);
              conversations.give_back_words(chat, std::mem::take(&mut took));
            }
          }
        }

        conversations.open_last(
          chat,
          drafted.round,
          drafted.privileged,
          drafted.tools_allowed,
          took.clone(),
        );
      } else {
        words = format!("{words}\n{}", waiting_line(&conversations.pending(chat)));
      }

      run.log.start(number, count, name);

      let asked = run.ask(Kind::Question, name, &words, last.then_some(took.len())).await;

      if last {
        let (grown, settled) = std::mem::take(&mut run.grown);

        took.extend(grown);
        settling.extend(settled);
      }

      match asked {
        Ok(Some((answer, read))) => {
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
          if last {
            conversations.give_back_words(chat, std::mem::take(&mut took));
            break;
          }

          // A question whose run failed is passed over to the last.
          at = count - 1;
          continue;
        }
        Err(failure) => {
          if last {
            conversations.give_back_words(chat, std::mem::take(&mut took));
          }

          return Err(failure);
        }
      }

      at += 1;
    }

    let past = |ceiling_at: i64| now_ms() >= ceiling_at;
    let owed = drafted.owed
      + if chat == owner {
        took.iter().filter(|message| message.sender_id == Some(owner)).count()
      } else {
        0
      };

    if skipped {
      message = None;
    }

    // The owner's own chat is always answered: a process that ended with no message over the owner's words is
    // asked once more, whatever the time.
    if message.is_none() && owed > 0 {
      let errand = fill_marks(&SECOND_ASKS.errands.owed, &[("count", &owed.to_string())]);

      if let Some((answer, read)) = run
        .ask(Kind::Item, "owed", &words(&errand, drafted.scope, &drafted.pending), None)
        .await?
        && usable(&answer.text)
        && read != Answered::Skip
      {
        message = Some(answer);
      }
    }

    let Some(mut message) = message else {
      let ended = if ceiling { Ended::CeilingSkip } else { Ended::Skip };

      run.log.end(ended, row, now_ms() - drafted.began).await;

      return Ok(made(&run.latest, SKIP, took, settling));
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
            .chain(took.iter())
            .any(|said| said.sender_id == Some(owner) && sets_rule_later(&said.text));

          (drafted.tools_allowed && set_one && !recorded(receiver, agent, &run.spoken, Some("rule")).await)
            .then(|| fill_marks(&SECOND_ASKS.errands.rule, &[("chat", &chat.to_string())]))
        }
      };
      let Some(errand) = errand else { continue };

      if let Some((answer, read)) = run
        .ask(Kind::Item, item.word(), &words(&errand, drafted.scope, &drafted.pending), None)
        .await?
      {
        if read == Answered::Skip {
          run.log.end(Ended::Skip, row, now_ms() - drafted.began).await;

          return Ok(made(&run.latest, SKIP, took, settling));
        }

        if usable(&answer.text) {
          message = answer;
        }
      }
    }

    // The checks on the final text, each asked once.
    let recent: Vec<String> = drafted.recent.iter().map(|message| message.text.clone()).collect();
    let terms = glossary(receiver);
    let mut asked: BTreeSet<Check> = BTreeSet::new();

    loop {
      let lines = own_lines(&run.asked);
      let reading = Reading {
        recent: &recent,
        glossary: &terms,
        own_lines: &lines,
        max_chars: DRAFTING.max_chars,
      };
      let failed = failing(&message.text, &reading);

      if failed.is_empty() {
        break;
      }

      if past(ceiling_at) || failed.iter().any(|check| asked.contains(check)) {
        let names: Vec<&str> = failed.iter().map(|check| check.word()).collect();

        eprintln!("chat {chat}: the message was withheld: {} failed", names.join(", "));
        run
          .log
          .end(Ended::Withheld, &format!("{row}; {}", names.join(", ")), now_ms() - drafted.began)
          .await;

        return Ok(made(&run.latest, SKIP, took, settling));
      }

      for check in failed {
        let errand = checks::errand(check, &message.text, &reading);

        asked.insert(check);

        if let Some((answer, read)) = run
          .ask(Kind::Check, check.word(), &words(&errand, drafted.scope, &drafted.pending), None)
          .await?
        {
          if read == Answered::Skip {
            run.log.end(Ended::Skip, row, now_ms() - drafted.began).await;

            return Ok(made(&run.latest, SKIP, took, settling));
          }

          if !is_wait(&answer.text) {
            message = answer;
          }
        }
      }
    }

    let ended = if ceiling { Ended::CeilingMessage } else { Ended::Message };

    run.log.end(ended, row, now_ms() - drafted.began).await;

    Ok(made(&run.latest, &message.text, took, settling))
  }
}

// What the process hands back: the text with what the last turn carried of the session, the words the last
// question took and the rows its pushes settled.
fn made(latest: &Reply, text: &str, took: Vec<Inbound>, settling: Vec<i64>) -> Made {
  Made {
    reply: Reply {
      text: text.to_owned(),
      ..latest.clone()
    },
    took,
    settling,
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

//! The draft process (docs/drafting.md): a round's message is made by asking its session the questions its row of
//! the table names, one turn each, the first riding the round's own turn; every answer before the last is notes,
//! and the last answer, or one that opens on the send marker, is the message. Then come the items the code's
//! readings call for and the checks on the final text, each asked once as one more turn. Every turn is written in
//! the round log.

pub mod checks;
pub mod log;
pub mod table;

use std::collections::{BTreeSet, HashMap, HashSet};

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::drafting::DRAFTING;
use receiver_policy::fill_marks;
use receiver_policy::second_asks::SECOND_ASKS;
use receiver_ports::gate::IndexEntry;
use serde_json::to_string as stringify;

use super::ask::Carry;
use super::conversation::{
  Asking, BoxFuture, Carried, Carrying, Conversations, Drafted, Drafting, Failure, Follower, Held, Made,
  Reply,
};
use super::mid_turn::{Push, paragraph};
use super::promises::{promises_after, promises_later, sets_rule_later};
use super::prompt::{Scope, scope_line};
use super::records::run_records;
use super::sentinels::{
  FOLD, Mark, SKIP, asks_fold, is_skip, is_wait, round_marks, sent_now, without_round_marks,
};
use super::turn_mark::{self, TurnMark};
use super::turnover::note_session;
use crate::form::strip_prompt_scaffolding;
use crate::now_ms;
use crate::one;
use crate::receiver::{Inbound, Receiver};

use checks::{Check, Reading, failing, has_skip, light, own_lines};
use log::{Answered, Ended, Kind, Log};
use table::{resolve, rows};

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
/// the note one turn's length is kept as, and whether the round's room has heard it is still at work. A round no
/// row matches asks nothing and has neither note nor bound.
#[derive(Clone, Debug, Default)]
pub struct Plan {
  pub questions: Vec<(String, String)>,
  pub send_from_at: usize,
  pub row: String,
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
      return fill_marks(&DRAFTING.send_open, &[("voice", &DRAFTING.voice)]);
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
    let lead = fill_marks(
      lead,
      &[
        ("number", "1"),
        ("count", &count.to_string()),
        ("name", name),
        ("send", &send),
        ("voice", &DRAFTING.voice),
      ],
    );

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
pub fn question_words(name: &str, text: &str, number: usize, count: usize, send: &str) -> String {
  let lead = if number == count { &DRAFTING.last_lead } else { &DRAFTING.lead };
  let lead = fill_marks(
    lead,
    &[
      ("number", &number.to_string()),
      ("count", &count.to_string()),
      ("name", name),
      ("send", send),
      ("voice", &DRAFTING.voice),
    ],
  );

  format!("{lead}\n\n{text}")
}

/// The line every question but the last closes on: the messages still waiting, shown with the next question.
pub fn waiting_line(pending: &[i64]) -> String {
  fill_marks(&DRAFTING.waiting, &[("ids", &stringify(&pending).unwrap_or_default())])
}

/// The closing check's turn (docs/drafting.md, 「Once the message stands, one closing check」): its line, the message
/// as it stands under its head, then the block of what arrived.
pub fn closing_words(standing: &str, block: &str) -> String {
  format!(
    "{}\n\n{}\n{}\n\n{block}",
    fill_marks(&DRAFTING.arrived, &[("voice", &DRAFTING.voice)]),
    DRAFTING.standing,
    standing.trim()
  )
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

/// What the session held as a round's turn went out: the session it continued, each path it held with the
/// version, and how each document of the turn's core went in by that.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Before {
  pub session: Option<String>,
  pub loads: HashMap<String, String>,
  pub core: HashMap<String, super::context::State>,
  /// What each document of the core holds, where it went named.
  pub names: HashMap<String, String>,
}

/// The pieces a turn's selection leaves out of its core because the session was handed all of them before, as the
/// round's log lists them: held by their path, with what they hold.
pub fn held_reads(held: &[(String, Option<String>)]) -> Vec<log::Read> {
  held
    .iter()
    .map(|(path, holds)| log::Read {
      path: path.clone(),
      holds: holds.clone(),
      memory_id: None,
      how: "held".to_owned(),
    })
    .collect()
}

/// The brain memory a piece read is, off the two path forms the gate builds for one memory: `memories/<id>` and
/// `memories/<room>/memory-<id>`. The room's due list, `memories/<room>`, is none.
pub fn memory_of(path: &str, chat_id: i64) -> Option<i64> {
  let rest = path.strip_prefix("memories/")?;

  match rest.split_once('/') {
    None => rest.parse::<i64>().ok().filter(|id| *id != chat_id),
    Some((room, slug)) => {
      room.parse::<i64>().ok()?;
      slug.strip_prefix("memory-")?.parse::<i64>().ok()
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
  turn_mark: TurnMark,
}

impl Run<'_> {
  // The process's end written down with what the round read and the session its turn ran on (docs/drafting.md,
  // 「The round log」).
  async fn end(&mut self, ended: Ended, detail: &str, ms: i64) {
    let reads = self.reads().await;
    let session = self.latest.session.clone();

    self.log.end(ended, detail, ms, &reads, session.as_deref()).await;
  }

  // What the round read from the brain (docs/drafting.md, 「The round log」): the core its turn carried, in full or
  // as a mark, then every piece the session read itself through the index, by the loads it gained or changed and
  // by the index reads its runs logged, a piece the session already held at that version standing as held. Each
  // is named by the index's words for it where the index lists it, else by the name it went in with, and carries
  // the brain memory it is.
  async fn reads(&self) -> Vec<log::Read> {
    let receiver = &self.drafter.receiver;
    let chat = self.held.chat_id;
    let before = &self.drafted.reply.before;
    let session = self.latest.session.as_deref();
    let after = super::context::loaded_in(receiver, session).await.unwrap_or_default();
    let earlier = if before.session.as_deref() == session {
      before.loads.clone()
    } else {
      Default::default()
    };
    let index: Vec<(String, IndexEntry)> = receiver
      .units()
      .gate
      .index(chat, session, Some(self.agent))
      .await
      .ok()
      .flatten()
      .unwrap_or_default()
      .into_iter()
      .flat_map(|group| {
        let name = group.group;

        group.entries.into_iter().map(move |entry| (name.clone(), entry))
      })
      .filter(|(_, entry)| !entry.path.is_empty())
      .collect();
    let named: HashMap<&str, &IndexEntry> =
      index.iter().map(|(_, entry)| (entry.path.as_str(), entry)).collect();
    let read = |path: &str, how: &str| log::Read {
      path: path.to_owned(),
      holds: named
        .get(path)
        .map(|entry| entry.holds.clone())
        .or_else(|| before.names.get(path).cloned()),
      memory_id: named.get(path).and_then(|entry| entry.memory_id).or_else(|| memory_of(path, chat)),
      how: how.to_owned(),
    };
    let mut core: Vec<(&String, &super::context::State)> = before.core.iter().collect();

    core.sort_by(|one, other| one.0.cmp(other.0));

    let mut reads: Vec<log::Read> = core
      .into_iter()
      .map(|(path, state)| {
        read(path, if *state == super::context::State::Held { "held" } else { "attached" })
      })
      .collect();

    // A piece the turn's core leaves out because the session was handed all of it before stands as held alone.
    reads.extend(
      held_reads(&self.drafted.selection.held)
        .into_iter()
        .filter(|read| !before.core.contains_key(&read.path)),
    );

    let outside = |path: &str| !before.core.contains_key(path) && !super::context::room_block(path);
    let mut pulled: BTreeSet<(String, &'static str)> = after
      .iter()
      .filter(|(path, version)| outside(path) && earlier.get(*path) != Some(*version))
      .map(|(path, _)| (path.clone(), "read"))
      .collect();
    let gained: HashSet<String> = pulled.iter().map(|(path, _)| path.clone()).collect();

    for path in self.logged_reads(&index).await {
      if !outside(&path) || gained.contains(&path) {
        continue;
      }

      let held = earlier
        .get(&path)
        .is_some_and(|version| after.get(&path).is_none_or(|now| now == version));

      pulled.insert((path, if held { "held" } else { "read" }));
    }

    reads.extend(pulled.into_iter().map(|(path, how)| read(&path, how)));
    reads
  }

  // The paths of the index's pieces the round's runs logged a read of: each call whose input names an entry of the
  // index by its group and slug.
  async fn logged_reads(&self, index: &[(String, IndexEntry)]) -> Vec<String> {
    let agency = &self.drafter.receiver.units().agency;
    let mut paths = vec![];

    for run in &self.spoken {
      for line in agency.run_log(run, None).await.unwrap_or_default() {
        if line.get("kind").and_then(serde_json::Value::as_str) != Some("tool") {
          continue;
        }

        let Some(input) = line
          .get("text")
          .and_then(serde_json::Value::as_str)
          .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        else {
          continue;
        };
        let field = |name: &str| input.get(name).and_then(serde_json::Value::as_str).map(str::to_owned);
        let (Some(group), Some(slug)) = (field("group"), field("slug")) else { continue };

        if let Some((_, entry)) = index.iter().find(|(named, entry)| *named == group && entry.slug == slug)
        {
          paths.push(entry.path.clone());
        }
      }
    }

    paths
  }

  // The review prompt (docs/drafting.md, 「The review」): the message as it stands, the room's words, what this
  // round asked and established as questions and the draft, and the rules and knowledge the round carries.
  fn review_prompt(&self, message: &str) -> String {
    let said = &DRAFTING.review;
    // What the main round handed is a line; what was not handed, or arrived since, is the body.
    let mut handed: Vec<i64> = vec![];
    let mut words: Vec<String> = vec![];

    for message in self.drafted.batch.iter().chain(self.shown.iter()) {
      if self.turn_mark.room(message.message_id) {
        if !handed.contains(&message.message_id) {
          handed.push(message.message_id);
        }
      } else {
        words.push(format!(
          "#{} {} (id {:?}): {}",
          message.message_id, message.sender_name, message.sender_id, message.text
        ));
      }
    }

    if !handed.is_empty() {
      let ids = handed.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(", ");

      words.insert(0, fill_marks(&said.room_handed, &[("ids", &ids)]));
    }

    let rules: Vec<String> = self
      .drafted
      .selection
      .entries
      .iter()
      .map(|entry| {
        if self.turn_mark.holds(&entry.path, &entry.text) {
          fill_marks(&said.handed, &[("path", &entry.path)])
        } else {
          format!("[{}]\n{}", entry.path, entry.text)
        }
      })
      .collect();
    let asked = self.asked.join("\n");

    format!(
      "{}\n\n{}\n{}\n\n{}\n{}\n\n{}\n{}\n\n{}\n{}\n\n{}",
      said.head,
      said.draft,
      message,
      said.words,
      words.join("\n"),
      said.notes,
      asked,
      said.rules,
      rules.join("\n\n"),
      render_verdict(&said.pass, &said.reject),
    )
  }

  // One more turn of the same session: its answer where it gave one, as the process reads it with its marks taken
  // out, none where its run failed, the stop raised. Nothing is pushed into it while it works.
  async fn ask(
    &mut self,
    kind: Kind,
    name: &str,
    words: &str,
  ) -> Result<Option<(Reply, Answered)>, Failure> {
    self.asked_with(kind, name, words, false).await
  }

  // One more turn of the same session that takes the room's words pushed into it while it works (docs/drafting.md,
  // 「What came in meanwhile」): what it took is shown to the round once it answers, and goes back to wait where
  // its run failed or the stop came.
  async fn ask_taking_words(
    &mut self,
    kind: Kind,
    name: &str,
    words: &str,
  ) -> Result<Option<(Reply, Answered)>, Failure> {
    self.asked_with(kind, name, words, true).await
  }

  async fn asked_with(
    &mut self,
    kind: Kind,
    name: &str,
    words: &str,
    taking: bool,
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
      thread_mark: None,
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

    if taking {
      self.conversations.carry_turn(
        chat,
        Carried::open(
          drafted.round,
          drafted.tools_allowed,
          drafted.privileged,
          false,
          self.holds.clone(),
          vec![],
        ),
      );
    }

    let carried = carrying.carry(&asking, &held, &follower).await;
    let pushed = follower.finish().map(Carried::into_shown).unwrap_or_default();

    self.only_marks = false;

    match carried {
      Ok(answered) => {
        self.shown.extend(pushed);

        let text = self.marked(&strip_prompt_scaffolding(&answered.text));
        // An answer of nothing but marks says nothing more: no skip, and no words.
        let (text, ending) = match (self.only_marks, text.is_empty()) {
          (true, _) => (text, Answered::Answered),
          (false, true) => read_answer(SKIP),
          (false, false) => read_answer(&text),
        };
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
        // A turn whose run failed showed nothing: what was pushed into it goes back to wait.
        self.conversations.give_back_words(chat, pushed);
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
  // An answer with the marks it sets on the messages shown to the round kept and taken out of it. The fold mark is
  // the message's own and stays in its words, but a fold asked of no words makes an answer of marks alone.
  fn marked(&mut self, text: &str) -> String {
    let marks = round_marks(text);
    let bare = without_round_marks(text);
    let folded = asks_fold(&bare);
    let bare = if folded && crate::js::trim(&bare.replace(FOLD, "")).is_empty() {
      String::new()
    } else {
      bare
    };

    self.only_marks = (!marks.is_empty() || folded) && bare.is_empty();
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
    let took = self.conversations.meanwhile(chat, drafted.privileged).await;

    if took.is_empty() {
      return None;
    }

    let built = paragraph(&Push {
      receiver,
      agent: self.agent,
      owner: receiver.owner_id(),
      chat_id: chat,
      round: drafted.round,
      messages: &took,
      instructions: &[],
      events: &[],
      holds: &self.holds,
      consumed: self.held.last_consumed_id,
      tools_allowed: drafted.tools_allowed,
      reader: Some(receiver.roads()),
      judged: false,
    })
    .await;

    match built {
      Ok(built) => {
        self.holds.extend(built.reference);
        self.conversations.took_meanwhile(chat, &took);

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
      reply_to: self.marks.iter().rev().find(|(mark, _)| *mark == Mark::Reply).map(|(_, id)| *id),
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
  // The process a round opens: the row its session's provider, any tier and its agent match, and the
  // questions that row asks of a turn that took what this one took.
  async fn plan(&self, agent: &str, chat: i64, batch: &[Inbound], instructions: usize) -> Plan {
    let receiver = &self.receiver;
    let provider = Carry::new(receiver.clone()).standing(agent, chat).await;
    let table = rows(receiver.db()).await.unwrap_or_else(|why| {
      eprintln!("chat {chat}: the draft process table could not be read: {}", why.detail);
      vec![]
    });
    let Some(process) = resolve(&table, provider.as_deref(), None, agent) else {
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
      shown: drafted.pushed.to_vec(),
      holds: drafted.selection.entries.iter().map(|entry| entry.path.clone()).collect(),
      marks: vec![],
      only_marks: false,
      turn_mark: TurnMark::default(),
    };
    // The main round handed the core and the room's words; the later rounds of this turn carry them as lines.
    let mut turn = turn_mark::read(receiver.db(), drafted.round).await;

    for entry in &drafted.selection.entries {
      turn.hand(&entry.path, &entry.text);
    }

    for message in drafted.batch {
      turn.hand_room(message.message_id);
    }

    if let Err(why) = turn_mark::write(receiver.db(), drafted.round, &turn).await {
      eprintln!("chat {chat}: the turn's mark was not kept: {}", why.detail);
    }

    run.turn_mark = turn;
    let first_text = run.marked(&drafted.reply.text);
    let (text, first_read) = read_answer(if first_text.is_empty() { SKIP } else { &first_text });
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

    let mut message: Option<Reply> = None;
    let mut skipped = first_read == Answered::Skip;

    // The first answer is the message where it sent one, or where the first question was the last or the turn
    // asked none; any other is notes.
    if !skipped && (first_read == Answered::Send || count <= 1) {
      message = Some(first);
    }

    let mut at = 1;

    while message.is_none() && !skipped && at < count {
      let number = at + 1;
      let last = number == count;
      let (name, text) = &plan.questions[at];
      let send = plan.send_line(at);
      let mut words = question_words(name, text, number, count, &send);

      run.asked.push(text.clone());

      // The room's words that waited since the last answer stand under this question.
      let block = run.arrived().await;

      if let Some((text, _)) = &block {
        words = format!("{words}\n\n{text}");
      }

      if !last {
        words = format!("{words}\n{}", waiting_line(&conversations.pending(chat)));
      }

      // The message round reads the room's situation note first (docs/drafting.md, 「The situation note」).
      if last && let Ok(Some(note)) = super::situation::read(receiver.db(), chat).await {
        words = format!("{}\n{note}\n\n{words}", CHAT_ROUND.situation.front);
      }

      run.log.start(number, count, name);

      let before = run.shown.len();
      let asked = run.ask_taking_words(Kind::Question, name, &words).await;
      let block = block.map(|(_, took)| took).unwrap_or_default();

      match asked {
        Ok(Some((answer, read))) => {
          // The block was shown ahead of what the turn took while it worked.
          run.shown.splice(before..before, block);

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
      && let Some((block, took)) = run.arrived().await
    {
      let mut words = closing_words(&standing.text, &block);

      if let Ok(Some(note)) = super::situation::read(receiver.db(), chat).await {
        words = format!("{}\n{note}\n\n{words}", CHAT_ROUND.situation.front);
      }

      match run.ask(Kind::Item, "arrived", &words).await {
        Ok(Some((answer, read))) => {
          run.shown.extend(took);

          if read == Answered::Skip {
            message = None;
            skipped = true;
          } else if is_wait(&answer.text) || run.only_marks || answer.text.trim().is_empty() {
            message = Some(standing.clone());
          } else {
            // The answer is the whole message as it now goes out, in the standing one's place.
            message = Some(answer);
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
      + if chat == receiver.owner_room() && !skipped {
        answered.iter().filter(|message| message.sender_id == Some(owner)).count()
      } else {
        0
      };
    // An owner's directive is answered only by a message the room's record holds. A note or an earlier draft that
    // says it was handled is not one, so a skip over such words is asked again (docs/drafting.md, 「The situation
    // note」 and 「What came in meanwhile」).
    let mut directed = false;

    for message in drafted.batch.iter().filter(|message| message.sender_id == Some(owner)) {
      if !answered_after(receiver, chat, receiver.roads().identity_in(chat).await, message.message_id).await
      {
        directed = true;
        break;
      }
    }

    // The owner's own chat is always answered: a process that ended with no message over the owner's words is
    // asked once more, whatever the time. So is any round carrying an owner's directive with no sent answer.
    if message.is_none() && (owed > 0 || directed) {
      let errand = if owed > 0 {
        fill_marks(&SECOND_ASKS.errands.owed, &[("count", &owed.to_string())])
      } else {
        SECOND_ASKS.errands.owner_directive.clone()
      };

      match run.ask(Kind::Item, "owed", &words(&errand, drafted.scope, &drafted.pending)).await {
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
      let ended = Ended::Skip;

      run.end(ended, row, now_ms() - drafted.began).await;

      return Ok(run.made(SKIP));
    };

    // The items the code's readings call for.
    for item in [Item::Record, Item::Wake, Item::Rule] {
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

      let asked = run.ask(Kind::Item, item.word(), &words(&errand, drafted.scope, &drafted.pending)).await;

      match asked {
        Ok(Some((answer, read))) => {
          if read == Answered::Skip {
            run.end(Ended::Skip, row, now_ms() - drafted.began).await;

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

    // The review first in the gate (docs/drafting.md, 「The review」): the message as it stands against the
    // room's words, what the round established and the rules it carries. A rejection returns it to `final` with
    // the reason, eight times at most; the eighth sends the last draft and the end row reports the reasons.
    let mut reasons: Vec<String> = vec![];
    let mut eight = false;
    let mut reviews = 0;

    while receiver.review_gate() && reviews < 8 {
      let started = now_ms();
      let answered = run.ask(Kind::Review, "review", &run.review_prompt(&message.text)).await?;
      let Some((answer, read)) = answered else {
        break;
      };
      let (pass, reason) = match read {
        Answered::Skip => (false, "the review gave no verdict".to_owned()),
        _ => review_verdict(&answer.text),
      };

      run
        .log
        .verdict("review", if pass { "PASS" } else { "REJECT" }, &reason, now_ms() - started)
        .await;

      for message in &run.shown {
        run.turn_mark.hand_room(message.message_id);
      }

      for message in drafted.batch {
        run.turn_mark.hand_room(message.message_id);
      }

      if let Err(why) = turn_mark::write(receiver.db(), drafted.round, &run.turn_mark).await {
        eprintln!("chat {chat}: the turn's mark was not kept: {}", why.detail);
      }

      reviews += 1;

      if pass {
        break;
      }

      reasons.push(reason.clone());

      if reviews >= 8 {
        eight = true;
        break;
      }

      let final_question = DRAFTING.questions.get("final").cloned().unwrap_or_default();
      let revise = fill_marks(&DRAFTING.review.revise, &[("reason", &reason)]);
      let question = format!("{final_question}\n\n{revise}");

      match run
        .ask(Kind::Question, "final", &words(&question, drafted.scope, &drafted.pending))
        .await
      {
        Ok(Some((answer, read))) => {
          if read == Answered::Skip {
            run.end(Ended::Skip, row, now_ms() - drafted.began).await;

            return Ok(run.made(SKIP));
          }

          message = answer;
        }
        Ok(None) => break,
        Err(failure) => {
          run.give_back_shown();

          return Err(failure);
        }
      }
    }

    // The checks on the final text, each asked once. The language an answer is written in is the agent's rule,
    // not a check: nothing on the send path reads it.
    let mut asked: HashSet<Check> = HashSet::new();
    loop {
      let lines = own_lines(&run.asked);
      let reading = Reading { own_lines: &lines };
      let failed = failing(&message.text, &reading);

      if failed.is_empty() {
        break;
      }

      let blocking: Vec<Check> = failed.to_vec();

      if blocking.iter().any(|check| asked.contains(check)) {
        let names: Vec<&str> = blocking.iter().map(|check| check.word()).collect();

        eprintln!("chat {chat}: the message was withheld: {} failed", names.join(", "));

        run
          .end(Ended::Withheld, &format!("{row}; {}", names.join(", ")), now_ms() - drafted.began)
          .await;

        return Ok(run.made(SKIP));
      }

      for check in failed {
        let errand = checks::errand(check, &message.text, &reading);

        asked.insert(check);

        let answered = run
          .ask(Kind::Check, check.word(), &words(&errand, drafted.scope, &drafted.pending))
          .await;

        match answered {
          Ok(Some((answer, read))) => {
            if read == Answered::Skip {
              run.end(Ended::Skip, row, now_ms() - drafted.began).await;

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

    let ended = Ended::Message;

    let detail = if eight {
      format!("{row}; review rejected eight times: {}", reasons.join(" | "))
    } else {
      row.to_owned()
    };

    run.end(ended, &detail, now_ms() - drafted.began).await;

    Ok(run.made(&message.text))
  }
}

/// The two words the review answer uses, as the policy names them.
fn render_verdict(pass: &str, reject: &str) -> String {
  format!("Answer {pass} or {reject}, and nothing else.")
}

/// Whether a message the agent itself sent follows the owner's directive in the room's record: the only thing that
/// makes an owner's words answered.
async fn answered_after(receiver: &Receiver, chat_id: i64, own: Option<i64>, message_id: i64) -> bool {
  let Some(own) = own else {
    return false;
  };
  let row = match one(
    receiver.db(),
    r#"SELECT COUNT(*) AS "sent" FROM "messages" m
       JOIN "messages" o ON o."chat_id" = m."chat_id" AND o."message_id" = $2
       WHERE m."chat_id" = $1 AND m."sender_id" = $3
         AND (m."sent_at", m."rowid") > (o."sent_at", o."rowid")"#,
    vec![chat_id.into(), message_id.into(), own.into()],
  )
  .await
  {
    Ok(Some(row)) => row,
    _ => return false,
  };

  row.try_get::<i64>("", "sent").unwrap_or(0) > 0
}

/// The verdict a review's answer gives: PASS, or REJECT with the reason it named; an answer naming neither is a
/// rejection carrying its own words.
fn review_verdict(text: &str) -> (bool, String) {
  let text = text.trim();

  if text.eq_ignore_ascii_case(DRAFTING.review.pass.trim()) || text.starts_with(DRAFTING.review.pass.trim())
  {
    return (true, String::new());
  }

  let reason = text
    .strip_prefix("REJECT")
    .map(|rest| rest.trim_start_matches(|one: char| one == ':' || one == '-' || one.is_whitespace()))
    .map(str::to_owned)
    .unwrap_or_else(|| text.to_owned());

  (false, reason.trim().to_owned())
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

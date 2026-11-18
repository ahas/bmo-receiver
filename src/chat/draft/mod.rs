//! The draft process (docs/drafting.md): a round's message is made by asking its session the questions its row of
//! the table names, one turn each, the first riding the round's own turn; every answer before the last is notes,
//! and the last answer, or one that opens on the send marker, is the message. Then come the items the code's
//! readings call for and the checks on the final text, each asked once as one more turn. A recording item answers
//! the short mark `CHECK` and writes no message, so the message that stands goes out unchanged. Every turn is
//! written in the round log.

pub mod checks;
pub mod log;
pub mod table;

use std::collections::{BTreeSet, HashMap, HashSet};

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
  FOLD, Mark, SKIP, Skip, asks_fold, is_skip, is_wait, round_marks, sent_now, skip, without_round_marks,
};
use super::turn_mark::{self, TurnMark};
use super::turnover::note_session;
use crate::form::strip_prompt_scaffolding;
use crate::now_ms;
use crate::one;
use crate::receiver::{Inbound, Receiver};

use checks::{Check, Reading, failing, own_lines};
use log::{Answered, Ended, Kind, Log, NO_REASON};

/// What an item or a check goes out with beside its marker and data: the turn's scope and the ids of the
/// messages waiting for the next turn. What is done with them stands in the room's block.
pub fn words(marker: &str, scope: &Scope, pending: &[i64]) -> String {
  format!(
    "{marker}\n{}\n{}{}",
    scope_line(scope),
    SECOND_ASKS.next_turn,
    stringify(&pending).unwrap_or_default(),
  )
}

/// The process a round opens before its turn goes out (docs/drafting.md, 「The process table」): the questions its
/// one draft turn follows, by name with their texts and in their order, and the row of the table it read. A round
/// no row matches follows none.
#[derive(Clone, Debug, Default)]
pub struct Plan {
  pub questions: Vec<(String, String)>,
  pub row: String,
  /// The stages after the message the row switches off (docs/drafting.md, 「The process table」).
  pub stages_off: Vec<String>,
}

impl Plan {
  /// Whether the round runs the stage of that name: every stage its row does not switch off.
  pub fn runs(&self, stage: &str) -> bool {
    !self.stages_off.iter().any(|named| named == stage)
  }

  /// The questions the round follows as its line and its row of the round log name them: in their order, joined
  /// by `+`; none where the round follows none.
  pub fn names(&self) -> Option<String> {
    (!self.questions.is_empty())
      .then(|| self.questions.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join("+"))
  }

  /// The line the round's turn closes on, where the round follows any question: the names of those it follows.
  /// Each question's own text stands in the room's block, never in the turn.
  pub fn first_words(&self) -> Option<String> {
    Some(fill_marks(&DRAFTING.round, &[("names", &self.names()?)]))
  }
}

/// An answer of a round's turn as the process reads it: a skip naming its reason standing anywhere in it outside
/// code, which wins and ends the round with nothing sent; a skip naming none, or no words at all, which is no
/// verdict and asks the same turn once more; a send marker opening it taken out, the words after it being the
/// answer, and a marker with nothing after it naming no reason; or an answer. The reason is what the round log
/// keeps where the turn skipped (docs/drafting.md, 「What the receiver reads of an answer」).
pub fn read_answer(text: &str) -> (String, Answered, Option<String>) {
  match skip(text) {
    Some(Skip::Reasoned(reason)) => return (text.to_owned(), Answered::Skip, Some(reason)),
    Some(Skip::Bare) => return (text.to_owned(), Answered::NoVerdict, None),
    None => {}
  }

  if is_skip(text) {
    return (text.to_owned(), Answered::NoVerdict, None);
  }

  match sent_now(text) {
    Some("") => (SKIP.to_owned(), Answered::NoVerdict, None),
    Some(words) => (words.to_owned(), Answered::Send, None),
    None => (text.to_owned(), Answered::Answered, None),
  }
}

/// One answer of the turns of the agent's program a turn of the process spanned, as the agency hands it: its
/// words, and whether words handed over opened that turn — the turn's own prompt or words pushed into it — or
/// the program opened it on its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spanned {
  pub text: String,
  pub handed: bool,
}

impl Spanned {
  /// Each answer of a turn's end as the agency hands it under `answers`; none where it hands none.
  pub fn of(ended: &serde_json::Value) -> Vec<Self> {
    ended
      .get("answers")
      .and_then(serde_json::Value::as_array)
      .map(|answers| {
        answers
          .iter()
          .map(|answer| Spanned {
            text: answer.get("text").and_then(serde_json::Value::as_str).unwrap_or_default().to_owned(),
            handed: answer.get("opened").and_then(serde_json::Value::as_str) != Some("program"),
          })
          .collect()
      })
      .unwrap_or_default()
  }
}

/// The answer of a turn of the process, built from the answers of the program's turns it spanned
/// (docs/drafting.md, 「What the receiver reads of an answer」): a turn opened by words handed over replaces it, a
/// turn the program opened on its own adds to it, and a turn whose answer says nothing never displaces an answer
/// that says something. Where the agency handed no turns, the answer it built stands.
pub fn spanned_answer(built: &str, turns: &[Spanned]) -> String {
  if turns.is_empty() {
    return built.to_owned();
  }

  turns.iter().filter(|turn| !turn.text.is_empty()).fold(String::new(), |standing, turn| {
    if usable(&standing) && !usable(&turn.text) {
      standing
    } else if standing.is_empty() || turn.handed {
      turn.text.clone()
    } else {
      format!("{standing}\n\n{}", turn.text)
    }
  })
}

/// A turn's answer as the process holds it: the reply, with what its run left of the session, how its row of the
/// round log ends and, where the turn skipped, the reason the log keeps.
#[derive(Clone, Debug)]
pub struct Answer {
  pub reply: Reply,
  pub ended: Answered,
  pub reason: Option<String>,
}

impl Answer {
  fn new(reply: Reply, ended: Answered, reason: Option<String>) -> Self {
    Answer {
      reply,
      ended,
      reason,
    }
  }

  // What the row of the round log says beside an ending of `skip`.
  fn detail(&self) -> Option<&str> {
    (self.ended == Answered::Skip).then_some(self.reason.as_deref()).flatten()
  }
}

/// The closing check's turn (docs/drafting.md, 「Once the message stands, one closing check」): its marker, the
/// message as it stands under its head, then the block of what arrived. What the check does stands in the room's
/// block.
pub fn closing_words(standing: &str, block: &str) -> String {
  format!("{}\n\n{}\n{}\n\n{block}", DRAFTING.arrived_marker, DRAFTING.standing, standing.trim())
}

/// An answer that says something: not empty, not a skip of either shape and not a wait.
pub fn usable(text: &str) -> bool {
  !text.is_empty() && skip(text).is_none() && !is_skip(text) && !is_wait(text)
}

/// The line a turn that skipped with no reason is asked once more with: the policy's own line over the turn as it
/// first went out.
fn over_skip(words: &str) -> String {
  format!("{}\n\n{words}", DRAFTING.skip_again)
}

/// An item's turn: the marker that names it and the data its rule reads.
pub fn item_words(name: &str, data: &[String]) -> String {
  let marker = fill_marks(&SECOND_ASKS.data.item, &[("name", name)]);

  std::iter::once(marker).chain(data.iter().cloned()).collect::<Vec<_>>().join("\n")
}

/// A check's turn: the marker that names it.
pub fn check_words(name: &str) -> String {
  fill_marks(&SECOND_ASKS.data.check, &[("name", name)])
}

/// Whether the turn's tools are on, as the data line an item reads it by.
pub fn tools_line(tools: bool) -> String {
  if tools {
    SECOND_ASKS.data.tools_on.clone()
  } else {
    SECOND_ASKS.data.tools_off.clone()
  }
}

pub fn record_errand(tools: bool) -> String {
  item_words("record", &[tools_line(tools)])
}

pub fn wake_errand(tools: bool, chat_id: i64) -> String {
  let room = fill_marks(&SECOND_ASKS.data.room, &[("chat", &chat_id.to_string())]);

  item_words("wake", &[tools_line(tools), room])
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
// what the round holds by path, the marks its answers set, whether the last answer was nothing but marks and the
// reason a skip of this round named.
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
  skip_reason: Option<String>,
}

impl Run<'_> {
  // The process's end written down with what the round read and the session its turn ran on (docs/drafting.md,
  // 「The round log」). Where the round ended in a skip, the row says why it sent nothing.
  async fn end(&mut self, ended: Ended, detail: &str, ms: i64) {
    let reads = self.reads().await;
    let session = self.latest.session.clone();
    let detail = match (ended, self.skip_reason.as_deref()) {
      (Ended::Skip, Some(reason)) => format!("{detail}; skipped: {reason}"),
      (Ended::Skip, None) => format!("{detail}; skipped: {NO_REASON}"),
      _ => detail.to_owned(),
    };

    self.log.end(ended, &detail, ms, &reads, session.as_deref()).await;
  }

  // The round skipped: the reason its log keeps, the one the answer named or none given.
  fn skipped(&mut self, reason: Option<&str>) {
    self.skip_reason = Some(reason.unwrap_or(NO_REASON).to_owned());
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

  // The review prompt (docs/drafting.md, 「The review」): its short mark, the questions the draft turn followed,
  // the message as it stands, the room's words and the rules and knowledge the round carries. What the review is
  // asked, its verdict line among it, stands in the room's block.
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

    // The questions the draft turn followed, as its line named them; a turn that followed none shows no line.
    let followed = self
      .drafted
      .plan
      .names()
      .map(|names| format!("{}\n", fill_marks(&said.followed, &[("names", &names)])))
      .unwrap_or_default();
    // The people the message answers, whose address terms the review holds it to; none shows no head.
    let people = self.answered_people();
    let answers = if people.is_empty() {
      String::new()
    } else {
      format!("{}\n{}\n\n", said.answers, people.join("\n"))
    };

    format!(
      "{}\n{followed}\n{}\n{}\n\n{}\n{}\n\n{answers}{}\n{}",
      said.marker,
      said.draft,
      message,
      said.words,
      words.join("\n"),
      said.rules,
      rules.join("\n\n"),
    )
  }

  // The people the message answers, one line each in the order they first spoke (docs/drafting.md, 「The review」):
  // every sender of the round's words and of those shown to it, but the agent itself and the senders of words its
  // answer marked as needing no answer or as answered in a round of their own.
  fn answered_people(&self) -> Vec<String> {
    let said = &DRAFTING.review;
    let set_aside =
      |id: i64| self.marks.contains(&(Mark::Done, id)) || self.marks.contains(&(Mark::Separate, id));
    let mut seen: Vec<Option<i64>> = vec![];
    let mut people = vec![];

    for message in self.drafted.batch.iter().chain(self.shown.iter()) {
      if message.sender_id == self.drafted.room.identity
        || set_aside(message.message_id)
        || seen.contains(&message.sender_id)
      {
        continue;
      }

      seen.push(message.sender_id);

      let id = message.sender_id.map_or_else(|| "unknown".to_owned(), |id| id.to_string());

      people.push(fill_marks(&said.person, &[("name", &message.sender_name), ("id", &id)]));
    }

    people
  }

  // One more turn of the same session: its answer where it gave one, none where its run failed, the stop raised.
  // Where the turn skipped naming no reason, the same turn is asked once more under the line naming the form, and
  // a second answer naming none is the round's own skip with its reason recorded as none given
  // (docs/drafting.md, 「What the receiver reads of an answer」). Nothing is pushed into a turn while it works.
  async fn ask(&mut self, kind: Kind, name: &str, words: &str) -> Result<Option<Answer>, Failure> {
    self.asked_with(kind, name, words, false).await
  }

  async fn asked_with(
    &mut self,
    kind: Kind,
    name: &str,
    words: &str,
    taking: bool,
  ) -> Result<Option<Answer>, Failure> {
    let Some(answer) = self.ask_turn(kind, name, words, taking, false).await? else {
      return Ok(None);
    };

    if answer.ended != Answered::NoVerdict {
      return Ok(Some(answer));
    }

    self.ask_turn(kind, name, &over_skip(words), taking, true).await
  }

  // One carrying of one turn of the process: its answer read with its marks taken out and its row written down.
  // `settling` names the asking a skip with no reason ends the round over, where no reason given is the reason.
  async fn ask_turn(
    &mut self,
    kind: Kind,
    name: &str,
    words: &str,
    taking: bool,
    settling: bool,
  ) -> Result<Option<Answer>, Failure> {
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
      standing: false,
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

        let said = strip_prompt_scaffolding(&answered.text);
        let text = self.marked(&said);
        // An answer of nothing but marks says nothing more: no skip, and no words.
        let (text, ending, reason) = match (self.only_marks, text.is_empty()) {
          (true, _) => (text, Answered::Answered, None),
          (false, true) => read_answer(SKIP),
          (false, false) => read_answer(&text),
        };
        // At the asking that settles the round, a skip naming no reason is its skip after all, none given.
        let (ending, reason) = match (ending, settling) {
          (Answered::NoVerdict, true) => (Answered::Skip, Some(NO_REASON.to_owned())),
          _ => (ending, reason),
        };
        let answer = Answer::new(Reply { text, ..answered }, ending, reason);

        if let Err(why) = note_session(
          &self.drafter.receiver,
          self.agent,
          chat,
          drafted.privileged,
          answer.reply.session.as_deref(),
          drafted.tools_allowed,
        )
        .await
        {
          eprintln!("chat {chat}: the session's launch: {}", why.detail);
        }

        self.spoken.extend(answer.reply.run.clone());
        self
          .log
          .turn(
            kind,
            name,
            Some(&answer.reply.text),
            answer.ended,
            answer.detail(),
            log::Timed::of(now_ms() - started, &answer.reply.spanned),
          )
          .await;
        self.latest = answer.reply.clone();

        Ok(Some(answer))
      }
      Err(failure) => {
        // A turn whose run failed showed nothing: what was pushed into it goes back to wait.
        self.conversations.give_back_words(chat, pushed);
        self
          .log
          .turn(kind, name, None, Answered::Failed, None, log::Timed::of(now_ms() - started, &[]))
          .await;

        // A turn the process cannot do without fails the round with it: the review, a revision and a check. Only
        // an item's turn lets the process go on without it (docs/drafting.md, 「The checks on the final text」).
        if matches!(failure, Failure::Stopped) || !matches!(kind, Kind::Item) {
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
      instruction_ids: &[],
      events: &[],
      holds: &self.holds,
      consumed: self.held.last_consumed_id,
      tools_allowed: drafted.tools_allowed,
      reader: Some(receiver.roads()),
      announced: false,
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
      own_done: self.own_marked(Mark::Done),
      own_separate: self.own_marked(Mark::Separate),
      reply_to: self.marks.iter().rev().find(|(mark, _)| *mark == Mark::Reply).map(|(_, id)| *id),
      silence: None,
    }
  }

  // What the process hands back where the round skipped: nothing to send, and the reason its log keeps.
  fn made_skip(&self) -> Made {
    self.made_silent(self.skip_reason.as_deref().unwrap_or(NO_REASON))
  }

  // What the process hands back where the round sends nothing, with why.
  fn made_silent(&self, why: &str) -> Made {
    Made {
      silence: Some(why.to_owned()),
      ..self.made(SKIP)
    }
  }

  // The round's own messages an answer marked `mark`, a message marked both done and separate standing as
  // separate alone.
  fn own_marked(&self, mark: Mark) -> Vec<Inbound> {
    let marked = |mark: Mark, id: i64| self.marks.contains(&(mark, id));

    self
      .drafted
      .batch
      .iter()
      .filter(|message| marked(mark, message.message_id))
      .filter(|message| mark == Mark::Separate || !marked(Mark::Separate, message.message_id))
      .cloned()
      .collect()
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
  // The process a round opens: the supervisor's row, its own or the policy's, and the questions that row asks of
  // a turn that took what this one took, in the supervisor's texts.
  async fn plan(&self, agent: &str, chat: i64) -> Plan {
    let receiver = &self.receiver;
    let row = table::row_of(receiver.db(), agent).await.unwrap_or_else(|why| {
      eprintln!("chat {chat}: the draft process table could not be read: {}", why.detail);
      None
    });
    let Some(process) = row else {
      return Plan {
        row: "none".to_owned(),
        ..Plan::default()
      };
    };
    // Every turn follows the same list, whatever it carried; a question the row has switched off keeps its place
    // in it and is not followed.
    let list = process.asked(&process.rounds);
    let catalogue = table::questions(receiver.db(), Some(agent)).await.unwrap_or_default();
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
      stages_off: process.stages_off.clone(),
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
    let mut run = Run {
      drafter: self,
      conversations,
      agent,
      held,
      drafted,
      spoken: drafted.reply.run.clone().into_iter().collect(),
      // The format check forbids the process's own lines in the final text: the texts of the questions the round
      // followed and the round's line.
      asked: {
        let mut asked: Vec<String> = plan.questions.iter().map(|(_, text)| text.clone()).collect();

        asked.extend(plan.first_words());

        asked
      },
      latest: drafted.reply.clone(),
      log: Log::new(conversations, drafted.round, chat, agent),
      shown: drafted.pushed.to_vec(),
      holds: drafted.selection.entries.iter().map(|entry| entry.path.clone()).collect(),
      marks: vec![],
      only_marks: false,
      turn_mark: TurnMark::default(),
      skip_reason: None,
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
    let (text, first_read, first_reason) =
      read_answer(if first_text.is_empty() { SKIP } else { &first_text });
    let mut first = Answer::new(
      Reply {
        text,
        ..drafted.reply.clone()
      },
      first_read,
      first_reason,
    );
    // The draft turn's row goes by the questions it followed; a turn that followed none is a plain turn.
    let names = plan.names();
    let (kind, name) = match &names {
      Some(names) => (Kind::Question, names.as_str()),
      None => (Kind::Turn, "turn"),
    };

    run.latest = first.reply.clone();
    run
      .log
      .turn(
        kind,
        name,
        Some(&first.reply.text),
        first.ended,
        first.detail(),
        log::Timed::of(now_ms() - drafted.began, &first.reply.spanned),
      )
      .await;

    // A draft turn that skipped naming no reason is the same turn asked once more under the line naming the form,
    // the session holding the round already, and a second answer naming none is the round's own skip.
    if first.ended == Answered::NoVerdict {
      let again = plan
        .first_words()
        .map(|line| over_skip(&line))
        .unwrap_or_else(|| DRAFTING.skip_again.clone());

      first = run
        .ask_turn(kind, name, &again, false, true)
        .await?
        .ok_or_else(|| Failure::Failed("the round's turn asked over its skip failed".to_owned()))?;
    }

    // The draft turn's answer is the message, unless it skipped.
    let mut skipped = first.ended == Answered::Skip;

    if skipped {
      run.skipped(first.reason.as_deref());
    }

    let mut message: Option<Reply> = (!skipped).then(|| first.reply.clone());

    // Once the message stands, the room's words that waited since the last answer are shown in one closing check.
    if let Some(standing) = &message
      && plan.runs("arrived")
      && let Some((block, took)) = run.arrived().await
    {
      let words = closing_words(&standing.text, &block);

      match run.ask(Kind::Item, "arrived", &words).await {
        Ok(Some(answer)) => {
          run.shown.extend(took);

          if answer.ended == Answered::Skip {
            message = None;
            skipped = true;
            run.skipped(answer.reason.as_deref());
          } else if is_wait(&answer.reply.text) || run.only_marks || answer.reply.text.trim().is_empty() {
            message = Some(standing.clone());
          } else {
            // The answer is the whole message as it now goes out, in the standing one's place.
            message = Some(answer.reply);
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
    let owed_item = if owed > 0 { "owed" } else { "owner_directive" };

    if message.is_none() && (owed > 0 || directed) && plan.runs(owed_item) {
      let errand = if owed > 0 {
        let count = fill_marks(&SECOND_ASKS.data.owed, &[("count", &owed.to_string())]);

        item_words("owed", &[count])
      } else {
        item_words("owner_directive", std::slice::from_ref(&SECOND_ASKS.data.owner_directive))
      };

      match run.ask(Kind::Item, "owed", &words(&errand, drafted.scope, &drafted.pending)).await {
        Ok(Some(answer)) if answer.ended == Answered::Skip => {
          run.skipped(answer.reason.as_deref());
        }
        Ok(Some(answer)) if usable(&answer.reply.text) => message = Some(answer.reply),
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

      return Ok(run.made_skip());
    };

    // The items the code's readings call for. Each asks only for the recording: its answer is the short mark
    // `CHECK`, never a new message, so the message that stands goes out unchanged (docs/drafting.md, 「The items
    // after the message」).
    for item in [Item::Record, Item::Wake, Item::Rule] {
      // An item the row switches off is not asked, whatever the reading of the message says.
      if !plan.runs(item.word()) {
        continue;
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
            .then(|| {
              let room = fill_marks(&SECOND_ASKS.data.room, &[("chat", &chat.to_string())]);

              item_words("rule", &[room])
            })
        }
      };
      let Some(errand) = errand else { continue };

      let asked = run.ask(Kind::Item, item.word(), &words(&errand, drafted.scope, &drafted.pending)).await;

      match asked {
        Ok(_) => {}
        Err(failure) => {
          run.give_back_shown();

          return Err(failure);
        }
      }
    }

    // The review first in the gate (docs/drafting.md, 「The review」): the message as it stands against the
    // room's words and the rules it carries. A rejection returns it to `final` with the reason, eight times at
    // most; the eighth sends the last draft and the end row reports the reasons. A rejection that names nothing
    // to change, or gives the reason it gave last, is no guidance: it sends the draft as it stands.
    let mut reasons: Vec<String> = vec![];
    let mut eight = false;
    let mut silent = false;
    let mut repeated = false;
    let mut previous: Option<String> = None;
    let mut reviews = 0;

    while receiver.review_gate() && reviews < 8 {
      let started = now_ms();
      let answered = run.ask(Kind::Review, "review", &run.review_prompt(&message.text)).await?;
      let Some(answer) = answered else {
        break;
      };
      // A skip in a review's answer is the review giving no verdict, never the round's own skip: where it names a
      // reason that reason rejects the draft, and where none was given even when asked over, the reason says so.
      // A review that writes the message again gives no reason: the draft unchanged passes, and a changed one takes
      // the draft's place and is reviewed again.
      let rewritten = (answer.ended == Answered::Send).then_some(&answer.reply);
      let (pass, reason) = match (answer.ended, answer.reason.as_deref()) {
        (Answered::Skip, Some(reason)) if reason != NO_REASON => (false, reason.to_owned()),
        (Answered::Skip, _) => (false, "the review gave no verdict".to_owned()),
        (Answered::Send, _) if onespace(&answer.reply.text) == onespace(&message.text) => {
          (true, String::new())
        }
        (Answered::Send, _) => (false, REWROTE.to_owned()),
        _ => review_verdict(&answer.reply.text),
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

      if let Some(rewrite) = rewritten {
        message = rewrite.clone();
      }

      if reviews >= 8 {
        eight = true;
        break;
      }

      if rewritten.is_some() {
        continue;
      }

      // A reason that names nothing is no guidance, and a reason that comes back unchanged means the revision
      // is not converging. Either way another round would repeat itself, so the draft as it stands is sent.
      if !names_a_change(&reason, &message.text) {
        silent = true;
        break;
      }

      let said = onespace(&reason);

      if previous.as_deref() == Some(said.as_str()) {
        repeated = true;
        break;
      }

      previous = Some(said);

      // The revision turn carries the reason as data; what to do with it stands in the room's block.
      let reason_shown = fill_marks(&DRAFTING.review.reason, &[("reason", &reason)]);

      match run
        .ask(Kind::Question, "final", &words(&reason_shown, drafted.scope, &drafted.pending))
        .await
      {
        Ok(Some(answer)) => {
          if answer.ended == Answered::Skip {
            run.skipped(answer.reason.as_deref());
            run.end(Ended::Skip, row, now_ms() - drafted.began).await;

            return Ok(run.made_skip());
          }

          message = answer.reply;
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
      let failed: Vec<Check> = failing(&message.text, &reading)
        .into_iter()
        .filter(|check| plan.runs(check.word()))
        .collect();

      if failed.is_empty() {
        break;
      }

      let blocking: Vec<Check> = failed.to_vec();

      if blocking.iter().any(|check| asked.contains(check)) {
        let names = blocking.iter().map(|check| check.word()).collect::<Vec<_>>().join(", ");

        eprintln!("chat {chat}: the message was withheld: {names} failed");

        run.end(Ended::Withheld, &format!("{row}; {names}"), now_ms() - drafted.began).await;

        return Ok(run.made_silent(&format!("withheld: {names}")));
      }

      for check in failed {
        let errand = checks::errand(check, &message.text, &reading);

        asked.insert(check);

        let answered = run
          .ask(Kind::Check, check.word(), &words(&errand, drafted.scope, &drafted.pending))
          .await;

        match answered {
          Ok(Some(answer)) => {
            if answer.ended == Answered::Skip {
              run.skipped(answer.reason.as_deref());
              run.end(Ended::Skip, row, now_ms() - drafted.began).await;

              return Ok(run.made_skip());
            }

            if !is_wait(&answer.reply.text) {
              message = answer.reply;
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
    } else if silent {
      format!("{row}; the review gave no usable reason, so the last draft was sent")
    } else if repeated {
      format!("{row}; the review gave the same reason twice, so the last draft was sent")
    } else {
      row.to_owned()
    };

    run.end(ended, &detail, now_ms() - drafted.began).await;

    Ok(run.made(&message.text))
  }
}

/// Whether a message the agent itself sent follows the owner's directive in the room's record: the only thing that
/// makes an owner's words answered. A thinking message is no such message.
async fn answered_after(receiver: &Receiver, chat_id: i64, own: Option<i64>, message_id: i64) -> bool {
  let Some(own) = own else {
    return false;
  };
  let row = match one(
    receiver.db(),
    r#"SELECT COUNT(*) AS "sent" FROM "messages" m
       JOIN "messages" o ON o."chat_id" = m."chat_id" AND o."message_id" = $2
       WHERE m."chat_id" = $1 AND m."sender_id" = $3 AND m."kind" IS DISTINCT FROM 'thinking'
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

/// The reason a review's row records where the review wrote the message again rather than giving a verdict.
pub const REWROTE: &str = "the review wrote the message again; the rewrite is reviewed as the draft";

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

/// Whether a review's reason gives the revision something to change: it is neither empty nor the draft's own
/// text repeated back.
fn names_a_change(reason: &str, draft: &str) -> bool {
  let reason = onespace(reason);

  !reason.is_empty() && reason != onespace(draft)
}

/// The text with its runs of white space collapsed to one space and its ends trimmed.
fn onespace(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Drafting for Drafter {
  fn plan<'a>(&'a self, agent: &'a str, chat_id: i64) -> BoxFuture<'a, Plan> {
    Box::pin(Drafter::plan(self, agent, chat_id))
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

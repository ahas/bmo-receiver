//! The instruction stages (docs/instruction.md): a round's message is made by the agent's own message turn — it reads
//! the room's words, works with its tools and answers with the message — and then goes through the chain of
//! instruction stages the supervisor sets in its settings, one system instruction a turn, each carrying its own
//! words: on a stage the agent answers with the whole message as it should now go out, or `PASS` alone when it
//! changes nothing and the message that stands goes on. The answer after the last stage is what is sent. A message
//! turn that sends nothing answers `<<SKIP: <why>>>` and ends the round with nothing sent. Every turn of the round is
//! written in the round log.

pub mod audit;
pub mod log;
pub mod table;

use std::collections::{BTreeSet, HashMap, HashSet};

use receiver_policy::fill_marks;
use receiver_policy::instruction::INSTRUCTION;
use receiver_ports::gate::IndexEntry;

use super::ask::Carry;
use super::conversation::{
  Asking, BoxFuture, Carried, Carrying, Conversations, Failure, Follower, Held, Made, MessageTurn,
  Pipeline, Reply,
};
use super::mid_turn::{Push, paragraph};
use super::sentinels::{
  FOLD, Mark, SKIP, Skip, asks_fold, is_skip, is_wait, round_marks, sent_now, skip, without_round_marks,
};
use super::turn_mark::{self, TurnMark};
use super::turnover::note_session;
use crate::form::strip_prompt_scaffolding;
use crate::now_ms;
use crate::receiver::{Inbound, Receiver};

use log::{Answered, Ended, Input, Kind, Log, NO_REASON, Started, tell_start};
use table::Stage;

/// An answer of a round's turn as the round reads it: a skip naming its reason standing anywhere in it outside
/// code, which wins and ends the round with nothing sent; a skip naming none, or no words at all, which is no
/// verdict and asks the same turn once more; a send marker opening it taken out, the words after it being the
/// answer, and a marker with nothing after it naming no reason; or an answer. The reason is what the round log
/// keeps where the turn skipped (docs/instruction.md, 「What it is」).
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
/// (docs/instruction.md, 「What it is」): a turn opened by words handed over replaces it, a
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

/// A turn's answer as the round holds it: the reply, with what its run left of the session, how its row of the
/// round log ends, where the turn skipped the reason the log keeps, and how long the turn took.
#[derive(Clone, Debug)]
pub struct Answer {
  pub reply: Reply,
  pub ended: Answered,
  pub reason: Option<String>,
  pub ms: i64,
}

impl Answer {
  fn new(reply: Reply, ended: Answered, reason: Option<String>, ms: i64) -> Self {
    Answer {
      reply,
      ended,
      reason,
      ms,
    }
  }

  // The answer's row of the round log, under its kind and name.
  async fn write(&self, log: &mut Log, kind: Kind, name: &str) {
    let timed = log::Timed::of(self.ms, &self.reply.spanned);

    log.turn(kind, name, Some(&self.reply.text), self.ended, self.detail(), timed).await;
  }

  // What the row of the round log says beside its ending: the reason a skip named, where the turn skipped.
  fn detail(&self) -> Option<&str> {
    self.reason.as_deref()
  }
}

/// The turn that shows the words that arrived meanwhile (docs/instruction.md, 「What came in meanwhile」): its
/// marker, then the block of what arrived. The message is the last one the session wrote in the round, so the turn
/// carries it no more; what it asks stands in the room's block.
pub fn closing_words(block: &str) -> String {
  format!("{}\n\n{block}", INSTRUCTION.arrived_marker)
}

/// One of the room's words as an instruction stage's turn shows it: its number, its sender and the sender's id where
/// the chat service gave one, then its text.
pub fn room_line(message: &Inbound) -> String {
  let sender = match message.sender_id {
    Some(id) => format!("{} (id {id})", message.sender_name),
    None => message.sender_name.clone(),
  };

  format!("#{} {sender}: {}", message.message_id, message.text)
}

/// An answer that says something: not empty, not a skip of either shape and not a wait.
pub fn usable(text: &str) -> bool {
  !text.is_empty() && skip(text).is_none() && !is_skip(text) && !is_wait(text)
}

/// The line a turn that skipped with no reason is asked once more with: the policy's own line over the turn as it
/// first went out.
fn over_skip(words: &str) -> String {
  format!("{}\n\n{words}", INSTRUCTION.skip_again)
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

/// The stages as the conversations hold them: the receiver they run on.
#[derive(Clone)]
pub struct Stager {
  receiver: Receiver,
}

impl Stager {
  pub fn new(receiver: Receiver) -> Self {
    Stager { receiver }
  }
}

// One round's stages as they run: the message turn they start from, the runs its turns opened, the reply the last
// turn carried and the log; the messages that arrived meanwhile and were shown to it, what the round holds by path,
// the marks its answers set, whether the last answer was nothing but marks and the reason a skip of this round named.
struct Run<'a> {
  stager: &'a Stager,
  conversations: &'a Conversations,
  agent: &'a str,
  held: &'a Held,
  message_turn: &'a MessageTurn<'a>,
  spoken: Vec<String>,
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
  // The round's end written down with what the round read and the session its turn ran on (docs/instruction.md,
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

  // What the round read from the brain (docs/instruction.md, 「The round log」): the core its turn carried, in full or
  // as a mark, then every piece the session read itself through the index, by the loads it gained or changed and
  // by the index reads its runs logged, a piece the session already held at that version standing as held. Each
  // is named by the index's words for it where the index lists it, else by the name it went in with, and carries
  // the brain memory it is.
  async fn reads(&self) -> Vec<log::Read> {
    let receiver = &self.stager.receiver;
    let chat = self.held.chat_id;
    let before = &self.message_turn.reply.before;
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
      held_reads(&self.message_turn.selection.held)
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
    let agency = &self.stager.receiver.units().agency;
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

  // An instruction stage's turn (docs/instruction.md, 「The stages before the send」): the line naming the stage, its
  // words, the room's words, the people the message answers and the rules and knowledge the round carries. The
  // message is the last one the session wrote in the round, so the turn carries it no more; how an instruction is
  // read and answered stands in the room's block.
  fn stage_prompt(&self, name: &str, text: &str) -> String {
    let said = &INSTRUCTION;
    // What the main round handed is a line; what was not handed, or arrived since, is the body.
    let mut handed: Vec<i64> = vec![];
    let mut words: Vec<String> = vec![];

    for message in self.message_turn.batch.iter().chain(self.shown.iter()) {
      if self.turn_mark.room(message.message_id) {
        if !handed.contains(&message.message_id) {
          handed.push(message.message_id);
        }
      } else {
        words.push(room_line(message));
      }
    }

    if !handed.is_empty() {
      let ids = handed.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(", ");

      words.insert(0, fill_marks(&said.room_handed, &[("ids", &ids)]));
    }

    let rules: Vec<String> = self
      .message_turn
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

    // The people the message answers, whose address terms the stage holds it to; none shows no head.
    let people = self.answered_people();
    let answers = if people.is_empty() {
      String::new()
    } else {
      format!("{}\n{}\n\n", said.answers, people.join("\n"))
    };

    format!(
      "{}\n{}\n\n{}\n{}\n\n{answers}{}\n{}",
      fill_marks(&said.marker, &[("name", name)]),
      text.trim(),
      said.words,
      words.join("\n"),
      said.rules,
      rules.join("\n\n"),
    )
  }

  // The people the message answers, in the order they first spoke (docs/instruction.md, 「The stages before the
  // send」): every sender of the round's words and of those shown to it, but the agent itself and the senders of
  // words its answer marked as needing no answer or as answered in a round of their own. Each is the name they go by
  // in the room's words and their sender number, where the chat service gave one.
  fn answered(&self) -> Vec<(String, Option<i64>)> {
    let set_aside =
      |id: i64| self.marks.contains(&(Mark::Done, id)) || self.marks.contains(&(Mark::Separate, id));
    let mut people: Vec<(String, Option<i64>)> = vec![];

    for message in self.message_turn.batch.iter().chain(self.shown.iter()) {
      if message.sender_id == self.message_turn.room.identity
        || set_aside(message.message_id)
        || people.iter().any(|(_, id)| *id == message.sender_id)
      {
        continue;
      }

      people.push((message.sender_name.clone(), message.sender_id));
    }

    people
  }

  // The people the message answers, one line each as a stage's turn shows them.
  fn answered_people(&self) -> Vec<String> {
    self
      .answered()
      .into_iter()
      .map(|(name, id)| {
        let id = id.map_or_else(|| "unknown".to_owned(), |id| id.to_string());

        fill_marks(&INSTRUCTION.person, &[("name", &name), ("id", &id)])
      })
      .collect()
  }

  // One more turn of the same session, its row written down. Where the turn skipped naming no reason, the same turn
  // is asked once more under the line naming the form, and a second answer naming none is the round's own skip with
  // its reason recorded as none given (docs/instruction.md, 「What it is」).
  async fn ask(&mut self, kind: Kind, name: &str, words: &str) -> Result<Answer, Failure> {
    let answer = self.carry(kind, name, words, false).await?;

    answer.write(&mut self.log, kind, name).await;

    if answer.ended != Answered::NoVerdict {
      return Ok(answer);
    }

    self.ask_over_skip(kind, name, &over_skip(words)).await
  }

  // The turn asked once more over a skip that named no reason: a skip naming none again is the round's skip.
  async fn ask_over_skip(&mut self, kind: Kind, name: &str, words: &str) -> Result<Answer, Failure> {
    let answer = self.carry(kind, name, words, true).await?;

    answer.write(&mut self.log, kind, name).await;

    Ok(answer)
  }

  // One carrying of one turn of the round: its answer read with its marks taken out, where the session's launch is
  // noted. A turn whose run failed writes its row as failed and fails the round, since no message goes out past a
  // turn it did not pass through (docs/instruction.md, 「The stages before the send」). `settling` names the asking a
  // skip with no reason ends the round over, where no reason given is the reason. Nothing is pushed into a later
  // turn while it works.
  async fn carry(
    &mut self,
    kind: Kind,
    name: &str,
    words: &str,
    settling: bool,
  ) -> Result<Answer, Failure> {
    let message_turn = self.message_turn;
    let chat = self.held.chat_id;
    let started = now_ms();
    let asking = Asking {
      chat_id: chat,
      session: self.held.session.clone().unwrap_or_default(),
      // A later turn of the round carries no reference of its own: the index went in with the message turn.
      prompt: words.to_owned(),
      via: match message_turn.via.as_str() {
        "" => None,
        road => Some(road.to_owned()),
      },
      privileged: message_turn.privileged,
      answers: vec![],
      began: started,
      context: message_turn.selection.context.clone(),
      thread_mark: None,
      standing: false,
    };
    let held = Held {
      agent: self.agent.to_owned(),
      chat_id: chat,
      session: self.latest.session.clone().or(self.held.session.clone()),
      account: self.held.account.clone(),
      context_tokens: self.latest.context_tokens.or(self.held.context_tokens),
      all_from_owner: message_turn.privileged,
      replaced: vec![],
      last_consumed_id: self.held.last_consumed_id,
    };
    let follower = Follower::new(self.conversations, chat);
    let carrying = Carry::new(self.stager.receiver.clone());
    let carried = carrying.carry(&asking, &held, &follower).await;
    let pushed = follower.finish().map(Carried::into_shown).unwrap_or_default();

    self.only_marks = false;

    let answered = match carried {
      Ok(answered) => answered,
      Err(failure) => {
        // A turn whose run failed showed nothing: what was pushed into it goes back to wait.
        self.conversations.give_back_words(chat, pushed);
        self
          .log
          .turn(kind, name, None, Answered::Failed, None, log::Timed::of(now_ms() - started, &[]))
          .await;

        return Err(failure);
      }
    };

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
    let answer = Answer::new(Reply { text, ..answered }, ending, reason, now_ms() - started);

    if let Err(why) = note_session(
      &self.stager.receiver,
      self.agent,
      chat,
      message_turn.privileged,
      answer.reply.session.as_deref(),
      message_turn.tools_allowed,
    )
    .await
    {
      eprintln!("chat {chat}: the session's launch: {}", why.detail);
    }

    self.spoken.extend(answer.reply.run.clone());
    self.latest = answer.reply.clone();

    Ok(answer)
  }

  // One instruction stage asked over the message as it stands, a stage of the chain or the audit, its row written
  // under `kind` with its verdict, and
  // the message that goes on from it: the stage's whole answer where it revised the message, the standing one where
  // it passed. The room's words and the rules it carried in full go to the later stages as marks.
  async fn stage(
    &mut self,
    kind: Kind,
    stage: &Stage,
    place: (usize, usize),
    message: Reply,
  ) -> Result<Reply, Failure> {
    let message_turn = self.message_turn;
    let receiver = &self.stager.receiver;
    let started = log::Started {
      number: place.0,
      count: place.1,
      question: &stage.name,
      input: Input::System,
    };

    tell_start(self.conversations, message_turn.round, self.held.chat_id, self.agent, started);

    let words = self.stage_prompt(&stage.name, &stage.text);
    let mut answer = self.carry(kind, &stage.name, &words, false).await?;
    let verdict = stage_verdict(&answer.reply.text, answer.ended, &message.text);
    let reason =
      (answer.ended == Answered::Skip).then(|| answer.reason.clone().unwrap_or(NO_REASON.to_owned()));

    answer.ended = verdict;
    answer.reason = reason;
    answer.write(&mut self.log, kind, &stage.name).await;

    for message in self.shown.iter().chain(message_turn.batch) {
      self.turn_mark.hand_room(message.message_id);
    }

    if let Err(why) = turn_mark::write(receiver.db(), message_turn.round, &self.turn_mark).await {
      eprintln!("chat {}: the turn's mark was not kept: {}", self.held.chat_id, why.detail);
    }

    Ok(if verdict == Answered::Revised { answer.reply } else { message })
  }
}

impl Run<'_> {
  // The audit of the people the message answers (docs/instruction.md, 「The audit before the stages」): asked where the
  // session read nothing from the brain about one of them, naming each such person with what the agent is to do,
  // and skipped where it read something of every one, where the brain does not reach the supervisor's turns or the
  // agent has no brain. The people it has the message ask how they want to be called are kept as asked, so none is
  // asked twice.
  async fn audit(&mut self, message: Reply) -> Result<Reply, Failure> {
    let receiver = self.stager.receiver.clone();
    let Some(brain) = receiver.units().brain.clone() else {
      return Ok(message);
    };

    if !super::window::brain_reaches(&receiver, self.agent).await {
      return Ok(message);
    }

    let read = self.people_read().await;
    let asked = audit::asked(receiver.db(), self.agent).await;
    let said = &INSTRUCTION.audit;
    let mut lines: Vec<String> = vec![];
    let mut asking: Vec<String> = vec![];

    for (name, id) in self.answered() {
      let Some(id) = id else { continue };
      let key = id.to_string();

      if read.contains(&key) {
        continue;
      }

      let known = match brain.person(self.agent, &key).await {
        Ok(Some(known)) => known,
        Ok(None) => return Ok(message),
        Err(why) => {
          eprintln!("chat {}: what the brain holds of {key} could not be read: {}", self.held.chat_id, why);
          continue;
        }
      };
      let owed = audit::owed(known, asked.contains(&key));
      let Some(line) = owed.line(said) else { continue };

      if owed.asks() {
        asking.push(key.clone());
      }

      lines.push(fill_marks(line, &[("name", &name), ("id", &key)]));
    }

    if lines.is_empty() {
      return Ok(message);
    }

    let stage = Stage {
      name: said.name.clone(),
      text: format!("{}\n{}", said.text, lines.join("\n")),
      set: false,
    };
    let message = self.stage(Kind::Audit, &stage, (1, 1), message).await?;

    if let Err(why) = audit::keep_asked(receiver.db(), self.agent, &asking).await {
      eprintln!("chat {}: the people asked their names were not kept: {}", self.held.chat_id, why.detail);
    }

    Ok(message)
  }

  // The people this session has read something of from the brain, by key: every piece its loads hold, the agency's
  // record of what it was handed and read through the gate, and the round's own core, read as a person's sheet by its
  // path or as a memory whose subjects the brain names.
  async fn people_read(&self) -> HashSet<String> {
    let receiver = &self.stager.receiver;
    let session = self.latest.session.as_deref();
    let mut paths: Vec<String> = super::context::loaded_in(receiver, session)
      .await
      .unwrap_or_default()
      .into_keys()
      .collect();

    paths.extend(self.message_turn.selection.entries.iter().map(|entry| entry.path.clone()));

    let mut read: HashSet<String> = paths.iter().filter_map(|path| audit::sheet_of(path)).collect();
    let memories: Vec<i64> = paths.iter().filter_map(|path| memory_of(path, self.held.chat_id)).collect();

    if let Some(brain) = receiver.units().brain.as_ref()
      && !memories.is_empty()
    {
      match brain.people_of(self.agent, &memories).await {
        Ok(people) => read.extend(people.unwrap_or_default()),
        Err(why) => eprintln!("chat {}: whom the memories read are about: {}", self.held.chat_id, why),
      }
    }

    read
  }
}

/// A stage's verdict on the message as it stood (docs/instruction.md, 「The stages before the send」): passed where it
/// answered `PASS` alone, the message unchanged, nothing but marks, a wait or a skip, since the round's skip
/// decision is its message turn's; revised where it wrote any other message, whose whole text takes the standing
/// one's place.
pub fn stage_verdict(said: &str, ended: Answered, standing: &str) -> Answered {
  let words = onespace(said);
  let unchanged = words.is_empty()
    || words.eq_ignore_ascii_case(&onespace(&INSTRUCTION.pass))
    || words == onespace(standing)
    || is_wait(said)
    || matches!(ended, Answered::Skip | Answered::NoVerdict);

  if unchanged { Answered::Passed } else { Answered::Revised }
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
  // (docs/instruction.md, 「What came in meanwhile」), with the words it shows; none where nothing waited, or where
  // the block could not be built and the words went back to wait.
  async fn arrived(&mut self) -> Option<(String, Vec<Inbound>)> {
    let message_turn = self.message_turn;
    let chat = self.held.chat_id;
    let receiver = &self.stager.receiver;
    let took = self.conversations.meanwhile(chat, message_turn.privileged).await;

    if took.is_empty() {
      return None;
    }

    let built = paragraph(&Push {
      receiver,
      agent: self.agent,
      owner: receiver.owner_id(),
      chat_id: chat,
      round: message_turn.round,
      messages: &took,
      instructions: &[],
      instruction_ids: &[],
      events: &[],
      holds: &self.holds,
      consumed: self.held.last_consumed_id,
      tools_allowed: message_turn.tools_allowed,
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
      .message_turn
      .batch
      .iter()
      .filter(|message| marked(mark, message.message_id))
      .filter(|message| mark == Mark::Separate || !marked(Mark::Separate, message.message_id))
      .cloned()
      .collect()
  }
}

impl Stager {
  // The instruction stages a round runs: the supervisor's chain, its own where the console set one and the
  // policy's where it did not, each stage with its words from the supervisor's texts. A supervisor whose chain is
  // empty runs no stage and sends its message as the message turn wrote it.
  async fn chain(&self, agent: &str) -> Vec<Stage> {
    let db = self.receiver.db();
    let texts = table::texts(db, Some(agent)).await.unwrap_or_default();

    table::chain(db, agent)
      .await
      .into_iter()
      .filter_map(|name| texts.iter().find(|stage| stage.name == name).cloned())
      .collect()
  }

  async fn run(
    &self,
    conversations: &Conversations,
    agent: &str,
    held: &Held,
    message_turn: &MessageTurn<'_>,
  ) -> Result<Made, Failure> {
    let receiver = &self.receiver;
    let chat = held.chat_id;
    let mut run = Run {
      stager: self,
      conversations,
      agent,
      held,
      message_turn,
      spoken: message_turn.reply.run.clone().into_iter().collect(),
      latest: message_turn.reply.clone(),
      log: Log::new(conversations, message_turn.round, chat, agent),
      shown: message_turn.pushed.to_vec(),
      holds: message_turn.selection.entries.iter().map(|entry| entry.path.clone()).collect(),
      marks: vec![],
      only_marks: false,
      turn_mark: TurnMark::default(),
      skip_reason: None,
    };
    // The message turn handed the core and the room's words; the later turns of this round carry them as lines.
    let mut turn = turn_mark::read(receiver.db(), message_turn.round).await;

    for entry in &message_turn.selection.entries {
      turn.hand(&entry.path, &entry.text);
    }

    for message in message_turn.batch {
      turn.hand_room(message.message_id);
    }

    if let Err(why) = turn_mark::write(receiver.db(), message_turn.round, &turn).await {
      eprintln!("chat {chat}: the turn's mark was not kept: {}", why.detail);
    }

    run.turn_mark = turn;
    let first_text = run.marked(&message_turn.reply.text);
    let (text, first_read, first_reason) =
      read_answer(if first_text.is_empty() { SKIP } else { &first_text });
    let mut first = Answer::new(
      Reply {
        text,
        ..message_turn.reply.clone()
      },
      first_read,
      first_reason,
      now_ms() - message_turn.began,
    );
    // The message turn's row of the log goes by its name.
    let name = Started::message().question;

    run.latest = first.reply.clone();
    first.write(&mut run.log, Kind::Turn, name).await;

    // A message turn that skipped naming no reason is asked once more under the line naming the form, the session
    // holding the round already, and a second answer naming none is the round's own skip.
    if first.ended == Answered::NoVerdict {
      first = run.ask_over_skip(Kind::Turn, name, &INSTRUCTION.skip_again).await?;
    }

    // The message turn's answer is the message, unless it skipped.
    if first.ended == Answered::Skip {
      run.skipped(first.reason.as_deref());
    }

    let mut message: Option<Reply> = (first.ended != Answered::Skip).then(|| first.reply.clone());

    // Once the message stands, the room's words that waited since the last answer are shown in one more system
    // instruction, so the message answers them as well (docs/instruction.md, 「What came in meanwhile」).
    if let Some(standing) = &message
      && let Some((block, took)) = run.arrived().await
    {
      let asked = closing_words(&block);
      let started = Started {
        question: log::ARRIVED,
        input: Input::System,
        ..Started::message()
      };

      tell_start(conversations, message_turn.round, chat, agent, started);

      match run.ask(Kind::Arrived, started.question, &asked).await {
        Ok(answer) => {
          run.shown.extend(took);

          if answer.ended == Answered::Skip {
            message = None;
            run.skipped(answer.reason.as_deref());
          } else if is_wait(&answer.reply.text) || run.only_marks || answer.reply.text.trim().is_empty() {
            message = Some(standing.clone());
          } else {
            // The answer is the whole message as it now goes out, in the standing one's place.
            message = Some(answer.reply);
          }
        }
        Err(failure) => {
          conversations.give_back_words(chat, took);
          run.give_back_shown();

          return Err(failure);
        }
      }
    }

    let Some(mut message) = message else {
      run.end(Ended::Skip, agent, now_ms() - message_turn.began).await;

      return Ok(run.made_skip());
    };

    // The audit of the people the message answers, then the instruction stages in the order the supervisor's chain
    // sets them, where the receiver runs them. A stage whose run failed fails the round, so no message goes out past a stage it did not pass through; the answer
    // after the last stage is what is sent.
    let gated = receiver.instruction_gate();

    if gated {
      match run.audit(message).await {
        Ok(next) => message = next,
        Err(failure) => {
          run.give_back_shown();

          return Err(failure);
        }
      }
    }

    let chain = if gated { self.chain(agent).await } else { vec![] };

    for (at, stage) in chain.iter().enumerate() {
      match run.stage(Kind::Stage, stage, (at + 1, chain.len()), message).await {
        Ok(next) => message = next,
        Err(failure) => {
          run.give_back_shown();

          return Err(failure);
        }
      }
    }

    run.end(Ended::Message, agent, now_ms() - message_turn.began).await;

    Ok(run.made(&message.text))
  }
}

/// The text with its runs of white space collapsed to one space and its ends trimmed.
fn onespace(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Pipeline for Stager {
  fn stages<'a>(
    &'a self,
    conversations: &'a Conversations,
    agent: &'a str,
    held: &'a Held,
    message_turn: &'a MessageTurn<'a>,
  ) -> BoxFuture<'a, Result<Made, Failure>> {
    Box::pin(self.run(conversations, agent, held, message_turn))
  }
}

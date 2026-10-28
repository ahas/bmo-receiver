//! The round log of the draft process (docs/drafting.md, 「The round log」): a row for every turn of a round's
//! process and one for its end, each told on the event stream as it is written, under the turn's speaker, and
//! read back by round.

use sea_orm::{ConnectionTrait, QueryResult};
use serde_json::{Value, json};

use crate::chat::conversation::Conversations;
use crate::receiver::Receiver;
use crate::store::Fault;
use crate::{all, now_ms, run};

/// A question of the process told as it starts, before its answer (docs/drafting.md, 「The round log」): its
/// place, how many questions the process asks and its name, told as the agent's own. It is no row of the log.
pub fn tell_start(
  conversations: &Conversations,
  round: &str,
  chat_id: i64,
  agent: &str,
  number: usize,
  count: usize,
  question: &str,
) {
  let member = json!({
    "round": round,
    "agentName": agent,
    "number": number,
    "count": count,
    "question": question,
    "kind": "start",
    "at": now_ms(),
  });

  conversations.publish(crate::events::draft(chat_id, &member));
}

/// What a row of the log is a row of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  /// The agent's own judgement whether to answer, before a round its open window alone let in.
  Supervisor,
  /// The round's own turn where the process asks no question, its answer being the message.
  Turn,
  /// A question of the process's list, the first riding the round's own turn.
  Question,
  /// An item asked once the message stands.
  Item,
  /// A check on the final text asked again.
  Check,
  /// The process's end.
  End,
}

impl Kind {
  pub fn word(self) -> &'static str {
    match self {
      Self::Supervisor => "supervisor",
      Self::Turn => "turn",
      Self::Question => "question",
      Self::Item => "item",
      Self::Check => "check",
      Self::End => "end",
    }
  }
}

/// How a turn of the process ended: answered, answered with the message now by the send marker, skipped or
/// failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answered {
  Answered,
  Send,
  Skip,
  Failed,
}

impl Answered {
  pub fn word(self) -> &'static str {
    match self {
      Self::Answered => "answered",
      Self::Send => "send",
      Self::Skip => "skip",
      Self::Failed => "failed",
    }
  }
}

/// How the process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
  Message,
  PastBound,
  Skip,
  Withheld,
}

impl Ended {
  pub fn word(self) -> &'static str {
    match self {
      Self::Message => "message",
      Self::PastBound => "past-bound",
      Self::Skip => "skip",
      Self::Withheld => "withheld",
    }
  }
}

/// One thing a round read from the brain (docs/drafting.md, 「The round log」): its path, what it holds, the brain
/// memory it is where it is one, and how it was read — `attached`, `held` or `read`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Read {
  pub path: String,
  pub holds: Option<String>,
  /// The brain memory the piece is, where it is one.
  #[serde(rename = "memoryId", default, skip_serializing_if = "Option::is_none")]
  pub memory_id: Option<i64>,
  pub how: String,
}

/// The log of one round's process as it is written: the round, the room and the agent, whose conversations tell
/// each row, the number the next row takes, counted from 1, and the answer the last turn gave, which the next is
/// compared with.
pub struct Log {
  conversations: Conversations,
  receiver: Receiver,
  round: String,
  chat_id: i64,
  agent: String,
  number: i64,
  last: Option<String>,
}

impl Log {
  pub fn new(conversations: &Conversations, round: &str, chat_id: i64, agent: &str) -> Self {
    Log {
      conversations: conversations.clone(),
      receiver: conversations.receiver(),
      round: round.to_owned(),
      chat_id,
      agent: agent.to_owned(),
      number: 1,
      last: None,
    }
  }

  /// The same log numbering its rows from `number`: the judgement before a round is its row 0.
  pub fn numbered_from(mut self, number: i64) -> Self {
    self.number = number;
    self
  }

  /// A question of the process told as it starts.
  pub fn start(&self, number: usize, count: usize, question: &str) {
    tell_start(&self.conversations, &self.round, self.chat_id, &self.agent, number, count, question);
  }

  /// A turn of the process written down: its kind, its question, its answer where it gave one, how it ended
  /// and how long it took.
  pub async fn turn(&mut self, kind: Kind, question: &str, answer: Option<&str>, ended: Answered, ms: i64) {
    let changed = match (answer, self.last.as_deref()) {
      (Some(answer), Some(last)) => answer.trim() != last.trim(),
      _ => false,
    };
    let row = Row {
      kind,
      question,
      answer,
      changed,
      outcome: ended.word(),
      detail: None,
      ms,
      reads: &[],
    };

    self.write(&row).await;

    if let Some(answer) = answer {
      self.last = Some(answer.to_owned());
    }
  }

  /// The process's end written down, with the row of the table it read, why it ended so and what the round read
  /// from the brain.
  pub async fn end(&mut self, ended: Ended, detail: &str, ms: i64, reads: &[Read]) {
    let row = Row {
      kind: Kind::End,
      question: "",
      answer: None,
      changed: false,
      outcome: ended.word(),
      detail: Some(detail),
      ms,
      reads,
    };

    self.write(&row).await;
  }

  async fn write(&mut self, row: &Row<'_>) {
    let at = now_ms();
    let number = self.number;
    let written = run(
      self.receiver.db(),
      r#"INSERT INTO "draft_rounds" ("round", "chat_id", "agent", "number", "question", "kind", "answer_chars",
           "changed", "outcome", "detail", "answer", "ms", "at", "reads")
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"#,
      vec![
        self.round.clone().into(),
        self.chat_id.into(),
        self.agent.clone().into(),
        number.into(),
        row.question.into(),
        row.kind.word().into(),
        row.chars().into(),
        row.changed.into(),
        row.outcome.into(),
        row.detail.map(str::to_owned).into(),
        row.answer.map(str::to_owned).into(),
        row.ms.into(),
        at.into(),
        (!row.reads.is_empty()).then(|| serde_json::to_string(row.reads).unwrap_or_default()).into(),
      ],
    )
    .await;

    self.number += 1;

    if let Err(why) = written {
      eprintln!("chat {}: a draft round could not be written down: {}", self.chat_id, why.detail);
    }

    let member = json!({
      "round": self.round,
      "agentName": self.agent,
      "number": number,
      "question": row.question,
      "kind": row.kind.word(),
      "answerChars": row.chars(),
      "changed": row.changed,
      "outcome": row.outcome,
      "detail": row.detail,
      "ms": row.ms,
      "at": at,
      "reads": row.reads,
    });

    self.conversations.publish(crate::events::draft(self.chat_id, &member));
  }
}

struct Row<'a> {
  kind: Kind,
  question: &'a str,
  answer: Option<&'a str>,
  changed: bool,
  outcome: &'a str,
  detail: Option<&'a str>,
  ms: i64,
  reads: &'a [Read],
}

impl Row<'_> {
  fn chars(&self) -> i64 {
    self.answer.map_or(0, |answer| answer.chars().count() as i64)
  }
}

/// A row of the log as the routes answer it.
pub fn member(row: &QueryResult) -> Result<Value, Fault> {
  Ok(json!({
    "id": row.try_get::<i64>("", "id")?,
    "agentName": row.try_get::<String>("", "agent")?,
    "number": row.try_get::<i64>("", "number")?,
    "question": row.try_get::<String>("", "question")?,
    "kind": row.try_get::<String>("", "kind")?,
    "answerChars": row.try_get::<i64>("", "answer_chars")?,
    "changed": row.try_get::<bool>("", "changed")?,
    "outcome": row.try_get::<String>("", "outcome")?,
    "detail": row.try_get::<Option<String>>("", "detail")?,
    "answer": row.try_get::<Option<String>>("", "answer")?,
    "ms": row.try_get::<i64>("", "ms")?,
    "at": row.try_get::<i64>("", "at")?,
    "reads": row
      .try_get::<Option<String>>("", "reads")?
      .and_then(|reads| serde_json::from_str::<Vec<Read>>(&reads).ok())
      .unwrap_or_default(),
  }))
}

/// The rows of the rounds named, each round's in the order they were written, a round with none not in it.
pub async fn read<C: ConnectionTrait>(
  db: &C,
  chat_id: i64,
  rounds: &[String],
) -> Result<Vec<Value>, Fault> {
  let mut read: Vec<(String, Vec<Value>)> = vec![];

  for round in rounds {
    let rows = all(
      db,
      r#"SELECT "id", "agent", "number", "question", "kind", "answer_chars", "changed", "outcome", "detail",
                "answer", "ms", "at", "reads"
           FROM "draft_rounds" WHERE "chat_id" = $1 AND "round" = $2 ORDER BY "id""#,
      vec![chat_id.into(), round.clone().into()],
    )
    .await?;

    if !rows.is_empty() {
      read.push((round.clone(), rows.iter().map(member).collect::<Result<Vec<Value>, Fault>>()?));
    }
  }

  Ok(read.into_iter().map(|(round, rows)| json!({ "round": round, "rows": rows })).collect())
}

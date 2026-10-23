//! The round log of the draft process (docs/drafting.md, 「The round log」): a row for every turn of a round's
//! process and one for its end, each told on the event stream as it is written, and read back by round.

use sea_orm::{ConnectionTrait, QueryResult};
use serde_json::{Value, json};

use crate::receiver::Receiver;
use crate::store::Fault;
use crate::{all, now_ms, run};

/// What a row of the log is a row of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  /// The round's own turn, whose answer is the draft.
  Turn,
  /// A question of the process's list.
  Question,
  /// An item asked after the last question.
  Item,
  /// A check on the final text asked again.
  Check,
  /// The process's end.
  End,
}

impl Kind {
  pub fn word(self) -> &'static str {
    match self {
      Self::Turn => "turn",
      Self::Question => "question",
      Self::Item => "item",
      Self::Check => "check",
      Self::End => "end",
    }
  }
}

/// How a turn of the process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answered {
  Answered,
  Skip,
  Failed,
}

impl Answered {
  pub fn word(self) -> &'static str {
    match self {
      Self::Answered => "answered",
      Self::Skip => "skip",
      Self::Failed => "failed",
    }
  }
}

/// How the process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
  Message,
  Skip,
  Withheld,
  CeilingMessage,
  CeilingSkip,
}

impl Ended {
  pub fn word(self) -> &'static str {
    match self {
      Self::Message => "message",
      Self::Skip => "skip",
      Self::Withheld => "withheld",
      Self::CeilingMessage => "ceiling-message",
      Self::CeilingSkip => "ceiling-skip",
    }
  }
}

/// The log of one round's process as it is written: the round, the room and the agent, the number the next row
/// takes and the answer the last turn gave, which the next is compared with.
pub struct Log {
  receiver: Receiver,
  round: String,
  chat_id: i64,
  agent: String,
  number: i64,
  last: Option<String>,
}

impl Log {
  pub fn new(receiver: &Receiver, round: &str, chat_id: i64, agent: &str) -> Self {
    Log {
      receiver: receiver.clone(),
      round: round.to_owned(),
      chat_id,
      agent: agent.to_owned(),
      number: 0,
      last: None,
    }
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
    };

    self.write(&row).await;

    if let Some(answer) = answer {
      self.last = Some(answer.to_owned());
    }
  }

  /// The process's end written down, with the row of the table it read and why it ended so.
  pub async fn end(&mut self, ended: Ended, detail: &str, ms: i64) {
    let row = Row {
      kind: Kind::End,
      question: "",
      answer: None,
      changed: false,
      outcome: ended.word(),
      detail: Some(detail),
      ms,
    };

    self.write(&row).await;
  }

  async fn write(&mut self, row: &Row<'_>) {
    let at = now_ms();
    let number = self.number;
    let written = run(
      self.receiver.db(),
      r#"INSERT INTO "draft_rounds" ("round", "chat_id", "agent", "number", "question", "kind", "answer_chars",
           "changed", "outcome", "detail", "answer", "ms", "at")
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"#,
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
    });

    self.receiver.news().publish(&crate::events::draft(self.chat_id, &member));
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
                "answer", "ms", "at"
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

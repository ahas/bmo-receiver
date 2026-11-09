//! The draft process table and its questions (docs/drafting.md, 「The process table」 and 「The questions」): the
//! policy's rows and catalogue, the console's rows and texts standing over them, the row a turn reads and what a
//! set may hold.

use std::collections::BTreeMap;

use receiver_policy::drafting::{DRAFTING, Process as PolicyProcess};
use sea_orm::{ConnectionTrait, QueryResult};
use serde_json::{Value, json};

use crate::store::Fault;
use crate::{all, now_ms, run};

/// The part of a row's key that stands for any provider, tier or agent.
pub const ANY: &str = "*";

/// The question every list that holds questions ends on.
pub const LAST: &str = "final";

/// The catalogue's name for the agent's own judgement of whether to answer, which opens the catalogue and which no
/// list may name (docs/drafting.md, 「The routes」).
pub const SUPERVISOR: &str = "supervisor";

/// The supervisor's text where the console set none: the round policy's head and question as one text.
pub fn supervisor_default() -> String {
  let said = &receiver_policy::chat_round::CHAT_ROUND.supervisor;

  format!("{}\n{}", said.head, said.question)
}

/// The longest a question's name may run.
pub const NAME_CHARS: usize = 40;

/// The longest a question's text may run.
pub const TEXT_CHARS: usize = 4000;

/// The longest ceiling a row may name, in seconds.
pub const LONGEST_CEILING: u32 = 3600;

/// The largest light bound a row may name, in characters.
pub const LARGEST_LIGHT: u32 = 1000;

/// What becomes of a turn once its ceiling has passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PastCeiling {
  /// What stands goes out.
  Send,
  /// Nothing goes out.
  Skip,
}

impl PastCeiling {
  pub fn word(self) -> &'static str {
    match self {
      Self::Send => "send",
      Self::Skip => "skip",
    }
  }

  pub fn of(word: &str) -> Option<Self> {
    match word {
      "send" => Some(Self::Send),
      "skip" => Some(Self::Skip),
      _ => None,
    }
  }
}

/// A row of the table: its key, each part a name or `*`, the questions of an ordinary and of a light turn, those
/// of them switched off, the question a send is read from, the light bound, the ceiling and what is done past it,
/// and whether the console set it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Process {
  pub provider: String,
  pub tier: String,
  pub agent: String,
  pub rounds: Vec<String>,
  /// The question a row once read a send from: kept as it came and read by nothing.
  pub send_from: String,
  pub light: Vec<String>,
  /// The questions of the two lists that are switched off: each keeps its place in its list and is not asked.
  pub off: Vec<String>,
  pub light_chars: u32,
  pub ceiling_seconds: u32,
  pub past_ceiling: PastCeiling,
  /// Whether the console set the row.
  pub set: bool,
}

impl Process {
  fn of_policy(row: &PolicyProcess) -> Self {
    Process {
      provider: row.provider.clone(),
      tier: row.tier.clone(),
      agent: row.agent.clone(),
      rounds: row.rounds.clone(),
      send_from: row.send_from.clone().unwrap_or_default(),
      light: row.light.clone(),
      off: row.off.clone(),
      light_chars: row.light_chars,
      ceiling_seconds: row.ceiling_seconds.unwrap_or(0),
      past_ceiling: row.past_ceiling.as_deref().and_then(PastCeiling::of).unwrap_or(PastCeiling::Send),
      set: false,
    }
    .settled()
  }

  /// The row with `off` naming only questions its lists hold, each once and in the lists' order: a name neither
  /// list holds is dropped.
  pub fn settled(mut self) -> Self {
    let mut off: Vec<String> = vec![];

    for name in self.rounds.iter().chain(&self.light) {
      if self.off.contains(name) && !off.contains(name) {
        off.push(name.clone());
      }
    }

    self.off = off;

    self
  }

  /// The questions of one of the row's lists a round follows: the list without those switched off, in its order.
  pub fn asked(&self, list: &[String]) -> Vec<String> {
    list.iter().filter(|name| !self.off.contains(name)).cloned().collect()
  }

  fn key(&self) -> (&str, &str, &str) {
    (&self.provider, &self.tier, &self.agent)
  }

  /// Whether the row is one a turn on this provider, tier and agent reads.
  pub fn matches(&self, provider: Option<&str>, tier: Option<&str>, agent: &str) -> bool {
    let fits = |part: &str, named: Option<&str>| part == ANY || named == Some(part);

    fits(&self.provider, provider) && fits(&self.tier, tier) && fits(&self.agent, Some(agent))
  }

  // How specific the row is: an agent's before a provider's, a provider's before a tier's.
  fn specificity(&self) -> u8 {
    u8::from(self.agent != ANY) * 4 + u8::from(self.provider != ANY) * 2 + u8::from(self.tier != ANY)
  }

  /// The row as the routes answer it.
  pub fn member(&self) -> Value {
    json!({
      "provider": self.provider,
      "tier": self.tier,
      "agentName": self.agent,
      "rounds": self.rounds,
      "sendFrom": self.send_from,
      "light": self.light,
      "off": self.off,
      "lightChars": self.light_chars,
      "ceilingSeconds": self.ceiling_seconds,
      "pastCeiling": self.past_ceiling.word(),
      "source": if self.set { "set" } else { "default" },
    })
  }
}

/// A question of the catalogue: its name, its text and whether the console set the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
  pub name: String,
  pub text: String,
  pub set: bool,
}

impl Question {
  /// The question as the routes answer it.
  pub fn member(&self) -> Value {
    json!({ "name": self.name, "text": self.text, "source": if self.set { "set" } else { "default" } })
  }
}

fn names(text: &str) -> Vec<String> {
  serde_json::from_str(text).unwrap_or_default()
}

fn set_row(row: &QueryResult) -> Result<Process, Fault> {
  let text = |column: &str| row.try_get::<String>("", column);
  let number = |column: &str| row.try_get::<i64>("", column);

  Ok(Process {
    provider: text("provider")?,
    tier: text("tier")?,
    agent: text("agent")?,
    rounds: names(&text("rounds")?),
    send_from: text("send_from")?,
    light: names(&text("light")?),
    off: names(&text("off")?),
    light_chars: u32::try_from(number("light_chars")?).unwrap_or(0),
    ceiling_seconds: u32::try_from(number("ceiling_seconds")?).unwrap_or(0),
    past_ceiling: PastCeiling::of(&text("past_ceiling")?).unwrap_or(PastCeiling::Send),
    set: true,
  })
}

/// Every row of the table: the policy's in their order, each replaced by the console's of the same key, then
/// the console's own keys.
pub async fn rows<C: ConnectionTrait>(db: &C) -> Result<Vec<Process>, Fault> {
  let read = all(
    db,
    r#"SELECT "provider", "tier", "agent", "rounds", "send_from", "light", "off", "light_chars",
            "ceiling_seconds", "past_ceiling"
         FROM "draft_processes" ORDER BY "provider", "tier", "agent""#,
    vec![],
  )
  .await?;
  let set = read
    .iter()
    .map(|row| set_row(row).map(Process::settled))
    .collect::<Result<Vec<Process>, Fault>>()?;
  let Some(default) = DRAFTING.process.first().map(Process::of_policy) else {
    return Ok(vec![]);
  };
  let universal = set.into_iter().find(|row| row.key() == default.key());

  Ok(vec![universal.unwrap_or(default)])
}

/// The row a turn reads: the most specific that matches its provider, its tier and its agent.
pub fn resolve(
  rows: &[Process],
  provider: Option<&str>,
  tier: Option<&str>,
  agent: &str,
) -> Option<Process> {
  rows
    .iter()
    .filter(|row| row.matches(provider, tier, agent))
    .fold(None::<&Process>, |best, row| match best {
      Some(best) if best.specificity() >= row.specificity() => Some(best),
      _ => Some(row),
    })
    .cloned()
}

/// The text the agent's own judgement of whether to answer opens on: the console's where it set one, else the
/// policy's.
pub async fn supervisor_text<C: ConnectionTrait>(db: &C) -> Result<String, Fault> {
  let read =
    all(db, r#"SELECT "text" FROM "draft_questions" WHERE "name" = $1"#, vec![SUPERVISOR.into()]).await?;

  match read.first() {
    Some(row) => Ok(row.try_get::<String>("", "text")?),
    None => Ok(supervisor_default()),
  }
}

/// The catalogue: the supervisor's text, then the policy's questions, each text replaced by the console's, then the
/// console's own names.
pub async fn questions<C: ConnectionTrait>(db: &C) -> Result<Vec<Question>, Fault> {
  let read = all(db, r#"SELECT "name", "text" FROM "draft_questions" ORDER BY "name""#, vec![]).await?;
  let mut set: BTreeMap<String, String> = BTreeMap::new();

  for row in read {
    set.insert(row.try_get::<String>("", "name")?, row.try_get::<String>("", "text")?);
  }

  let stands = |name: &str, text: &str, set: &mut BTreeMap<String, String>| match set.remove(name) {
    Some(text) => Question {
      name: name.to_owned(),
      text,
      set: true,
    },
    None => Question {
      name: name.to_owned(),
      text: text.to_owned(),
      set: false,
    },
  };
  let mut questions = vec![stands(SUPERVISOR, &supervisor_default(), &mut set)];

  for (name, text) in &DRAFTING.questions {
    questions.push(stands(name, text, &mut set));
  }

  questions.extend(set.into_iter().map(|(name, text)| Question {
    name,
    text,
    set: true,
  }));

  Ok(questions)
}

/// Why a row may not be set, where it may not; the catalogue's names are the questions a list may name.
pub fn refusal(process: &Process, catalogue: &[Question]) -> Option<String> {
  for (what, list) in [("rounds", &process.rounds), ("light", &process.light)] {
    if list.iter().any(|name| name == SUPERVISOR) {
      return Some(format!("{what} names {SUPERVISOR}, which is asked before any process and in no list."));
    }

    if let Some(unknown) =
      list.iter().find(|name| !catalogue.iter().any(|question| &question.name == *name))
    {
      return Some(format!("{what} names the question {unknown}, which the catalogue does not hold."));
    }

    let finals = list.iter().filter(|name| name.as_str() == LAST).count();

    if !list.is_empty() && (list.last().map(String::as_str) != Some(LAST) || finals != 1) {
      return Some(format!("{what} must end in {LAST} and hold it nowhere else."));
    }
  }

  if process.off.iter().any(|name| name == LAST) {
    return Some(format!("off names {LAST}, which writes the message and is never switched off."));
  }

  if process.light_chars > LARGEST_LIGHT {
    return Some(format!("lightChars must be from 0 to {LARGEST_LIGHT}."));
  }

  None
}

/// Why a question's name or text may not be set, where it may not.
pub fn question_refusal(name: &str, text: &str) -> Option<String> {
  let named = name.chars().count() <= NAME_CHARS
    && name.chars().next().is_some_and(|first| first.is_ascii_lowercase())
    && name.chars().all(|one| one.is_ascii_lowercase() || one.is_ascii_digit() || one == '-');

  if !named {
    return Some(format!(
      "A question's name is a lowercase word of letters, digits and dashes of at most {NAME_CHARS} characters."
    ));
  }

  if text.trim().is_empty() || text.chars().count() > TEXT_CHARS {
    return Some(format!("A question's text is not empty and at most {TEXT_CHARS} characters."));
  }

  None
}

/// A row set by the console, over whatever stood at its key.
pub async fn set<C: ConnectionTrait>(db: &C, process: &Process) -> Result<(), Fault> {
  let listed = |list: &[String]| serde_json::to_string(list).unwrap_or_else(|_| "[]".to_owned());

  run(
    db,
    r#"INSERT INTO "draft_processes"
         ("provider", "tier", "agent", "rounds", "send_from", "light", "off", "light_chars", "ceiling_seconds",
          "past_ceiling", "set_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
       ON CONFLICT ("provider", "tier", "agent") DO UPDATE SET
         "rounds" = EXCLUDED."rounds", "send_from" = EXCLUDED."send_from", "light" = EXCLUDED."light",
         "off" = EXCLUDED."off",
         "light_chars" = EXCLUDED."light_chars", "ceiling_seconds" = EXCLUDED."ceiling_seconds",
         "past_ceiling" = EXCLUDED."past_ceiling",
         "set_at" = EXCLUDED."set_at""#,
    vec![
      process.provider.clone().into(),
      process.tier.clone().into(),
      process.agent.clone().into(),
      listed(&process.rounds).into(),
      process.send_from.clone().into(),
      listed(&process.light).into(),
      listed(&process.off).into(),
      i64::from(process.light_chars).into(),
      i64::from(process.ceiling_seconds).into(),
      process.past_ceiling.word().into(),
      now_ms().into(),
    ],
  )
  .await
  .map(drop)
}

/// The console's row of a key taken away; whether there was one.
pub async fn remove<C: ConnectionTrait>(
  db: &C,
  provider: &str,
  tier: &str,
  agent: &str,
) -> Result<bool, Fault> {
  let gone = run(
    db,
    r#"DELETE FROM "draft_processes" WHERE "provider" = $1 AND "tier" = $2 AND "agent" = $3"#,
    vec![provider.into(), tier.into(), agent.into()],
  )
  .await?;

  Ok(gone > 0)
}

/// A question's text set by the console.
pub async fn set_question<C: ConnectionTrait>(db: &C, name: &str, text: &str) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "draft_questions" ("name", "text", "set_at") VALUES ($1, $2, $3)
       ON CONFLICT ("name") DO UPDATE SET "text" = EXCLUDED."text", "set_at" = EXCLUDED."set_at""#,
    vec![name.into(), text.into(), now_ms().into()],
  )
  .await
  .map(drop)
}

/// Whether the policy's catalogue holds a question of the name, the agent's own judgement among them: one whose
/// text stands again in the policy's once the console's is taken away.
pub fn policy_holds(name: &str) -> bool {
  name == SUPERVISOR || DRAFTING.questions.contains_key(name)
}

/// The console's text of a question taken away; whether there was one.
pub async fn remove_question<C: ConnectionTrait>(db: &C, name: &str) -> Result<bool, Fault> {
  let gone = run(db, r#"DELETE FROM "draft_questions" WHERE "name" = $1"#, vec![name.into()]).await?;

  Ok(gone > 0)
}

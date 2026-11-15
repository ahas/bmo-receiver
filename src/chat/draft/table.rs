//! The draft process table and its questions (docs/drafting.md, 「The process table」 and 「The questions」): the
//! policy's row and catalogue, each supervisor's own row and texts standing over them for that supervisor alone,
//! the row a turn reads and what a set may hold.

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

/// The stages after the message a row may switch off, in the order a settled row names them: the review, the
/// items after the message and the checks on the final text (docs/drafting.md, 「The process table」).
pub const STAGES: [&str; 8] =
  ["arrived", "owed", "owner_directive", "record", "wake", "rule", "review", "format"];

/// The longest ceiling a row may name, in seconds.
pub const LONGEST_CEILING: u32 = 3600;

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

/// A row of the table: its key, the policy's `*/*/*` or a supervisor's name under any provider and tier, the
/// questions every turn's round follows, those of them switched off, the question a send is read from, the
/// ceiling and what is done past it, and whether the console set it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Process {
  pub provider: String,
  pub tier: String,
  pub agent: String,
  pub rounds: Vec<String>,
  /// The question a row once read a send from: kept as it came and read by nothing.
  pub send_from: String,
  /// The questions of the list that are switched off: each keeps its place in it and is not asked.
  pub off: Vec<String>,
  /// The stages after the message that are switched off, out of `STAGES`, in its order.
  pub stages_off: Vec<String>,
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
      off: row.off.clone(),
      stages_off: row.stages_off.clone(),
      ceiling_seconds: row.ceiling_seconds.unwrap_or(0),
      past_ceiling: row.past_ceiling.as_deref().and_then(PastCeiling::of).unwrap_or(PastCeiling::Send),
      set: false,
    }
    .settled()
  }

  /// The row with `off` naming only questions its list holds, each once and in the list's order: a name the list
  /// does not hold is dropped.
  pub fn settled(mut self) -> Self {
    let mut off: Vec<String> = vec![];

    for name in &self.rounds {
      if self.off.contains(name) && !off.contains(name) {
        off.push(name.clone());
      }
    }

    self.off = off;
    self.stages_off = STAGES
      .iter()
      .filter(|stage| self.stages_off.iter().any(|named| named == *stage))
      .map(|stage| (*stage).to_owned())
      .collect();

    self
  }

  /// Whether the row switches off the stage of that name.
  pub fn stage_off(&self, stage: &str) -> bool {
    self.stages_off.iter().any(|named| named == stage)
  }

  /// The questions of the row's list a round follows: the list without those switched off, in its order.
  pub fn asked(&self, list: &[String]) -> Vec<String> {
    list.iter().filter(|name| !self.off.contains(name)).cloned().collect()
  }

  /// Whether the row's key is a supervisor's own, the only kind the console sets: a supervisor's name under any
  /// provider and any tier.
  pub fn own(&self) -> bool {
    self.provider == ANY && self.tier == ANY && self.agent != ANY && !self.agent.trim().is_empty()
  }

  /// The row as the routes answer it.
  pub fn member(&self) -> Value {
    json!({
      "provider": self.provider,
      "tier": self.tier,
      "agentName": self.agent,
      "rounds": self.rounds,
      "sendFrom": self.send_from,
      "off": self.off,
      "stagesOff": self.stages_off,
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
    off: names(&text("off")?),
    stages_off: names(&text("stages_off")?),
    ceiling_seconds: u32::try_from(number("ceiling_seconds")?).unwrap_or(0),
    past_ceiling: PastCeiling::of(&text("past_ceiling")?).unwrap_or(PastCeiling::Send),
    set: true,
  })
}

/// The policy's row, which a supervisor reads until the console sets it one of its own.
pub fn policy_row() -> Option<Process> {
  DRAFTING.process.first().map(Process::of_policy)
}

/// Every row of the table: the policy's, then each supervisor's own by the supervisor's name.
pub async fn rows<C: ConnectionTrait>(db: &C) -> Result<Vec<Process>, Fault> {
  let read = all(
    db,
    r#"SELECT "provider", "tier", "agent", "rounds", "send_from", "off", "stages_off", "ceiling_seconds",
              "past_ceiling"
         FROM "draft_processes" ORDER BY "agent""#,
    vec![],
  )
  .await?;
  let own = read
    .iter()
    .map(|row| set_row(row).map(Process::settled))
    .collect::<Result<Vec<Process>, Fault>>()?;

  Ok(policy_row().into_iter().chain(own.into_iter().filter(Process::own)).collect())
}

/// The row a supervisor's turn reads: its own where the console set it one, else the policy's.
pub fn resolve(rows: &[Process], agent: &str) -> Option<Process> {
  rows
    .iter()
    .find(|row| row.own() && row.agent == agent)
    .or_else(|| rows.iter().find(|row| !row.own()))
    .cloned()
}

/// The row a supervisor's turn reads, read from the table.
pub async fn row_of<C: ConnectionTrait>(db: &C, agent: &str) -> Result<Option<Process>, Fault> {
  Ok(resolve(&rows(db).await?, agent))
}

/// The text a supervisor's own judgement of whether to answer opens on: the console's where it set that
/// supervisor one, else the policy's.
pub async fn supervisor_text<C: ConnectionTrait>(db: &C, agent: &str) -> Result<String, Fault> {
  let read = all(
    db,
    r#"SELECT "text" FROM "draft_questions" WHERE "agent" = $1 AND "name" = $2"#,
    vec![agent.into(), SUPERVISOR.into()],
  )
  .await?;

  match read.first() {
    Some(row) => Ok(row.try_get::<String>("", "text")?),
    None => Ok(supervisor_default()),
  }
}

/// A supervisor's catalogue: its first judgement's text, then the policy's questions, each text replaced by the
/// one the console set that supervisor, then the names the console added for it. With no supervisor named, the
/// policy's alone.
pub async fn questions<C: ConnectionTrait>(db: &C, agent: Option<&str>) -> Result<Vec<Question>, Fault> {
  let read = match agent {
    Some(agent) => {
      all(
        db,
        r#"SELECT "name", "text" FROM "draft_questions" WHERE "agent" = $1 ORDER BY "name""#,
        vec![agent.into()],
      )
      .await?
    }
    None => vec![],
  };
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
  if !process.own() {
    return Some(format!(
      "A draft process row is one supervisor's: its key is {ANY}/{ANY}/ and the supervisor's name."
    ));
  }

  let list = &process.rounds;

  if list.iter().any(|name| name == SUPERVISOR) {
    return Some(format!("rounds names {SUPERVISOR}, which is asked before any process and in no list."));
  }

  if let Some(unknown) = list.iter().find(|name| !catalogue.iter().any(|question| &question.name == *name))
  {
    return Some(format!("rounds names the question {unknown}, which the catalogue does not hold."));
  }

  let finals = list.iter().filter(|name| name.as_str() == LAST).count();

  if !list.is_empty() && (list.last().map(String::as_str) != Some(LAST) || finals != 1) {
    return Some(format!("rounds must end in {LAST} and hold it nowhere else."));
  }

  if process.off.iter().any(|name| name == LAST) {
    return Some(format!("off names {LAST}, which writes the message and is never switched off."));
  }

  if let Some(unknown) = process.stages_off.iter().find(|name| !STAGES.contains(&name.as_str())) {
    return Some(format!(
      "stagesOff names {unknown}, which is no stage after the message; the stages are {}.",
      STAGES.join(", ")
    ));
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
         ("provider", "tier", "agent", "rounds", "send_from", "off", "stages_off", "ceiling_seconds",
          "past_ceiling", "set_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
       ON CONFLICT ("provider", "tier", "agent") DO UPDATE SET
         "rounds" = EXCLUDED."rounds", "send_from" = EXCLUDED."send_from", "off" = EXCLUDED."off",
         "stages_off" = EXCLUDED."stages_off",
         "ceiling_seconds" = EXCLUDED."ceiling_seconds",
         "past_ceiling" = EXCLUDED."past_ceiling",
         "set_at" = EXCLUDED."set_at""#,
    vec![
      process.provider.clone().into(),
      process.tier.clone().into(),
      process.agent.clone().into(),
      listed(&process.rounds).into(),
      process.send_from.clone().into(),
      listed(&process.off).into(),
      listed(&process.stages_off).into(),
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

/// A question's text set by the console for a supervisor.
pub async fn set_question<C: ConnectionTrait>(
  db: &C,
  agent: &str,
  name: &str,
  text: &str,
) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "draft_questions" ("agent", "name", "text", "set_at") VALUES ($1, $2, $3, $4)
       ON CONFLICT ("agent", "name") DO UPDATE SET "text" = EXCLUDED."text", "set_at" = EXCLUDED."set_at""#,
    vec![agent.into(), name.into(), text.into(), now_ms().into()],
  )
  .await
  .map(drop)
}

/// Everything the console set a supervisor taken away with the supervisor: its row and its question texts.
pub async fn forget<C: ConnectionTrait>(db: &C, agent: &str) -> Result<(), Fault> {
  run(db, r#"DELETE FROM "draft_processes" WHERE "agent" = $1"#, vec![agent.into()]).await?;
  run(db, r#"DELETE FROM "draft_questions" WHERE "agent" = $1"#, vec![agent.into()]).await?;

  Ok(())
}

/// Whether the policy's catalogue holds a question of the name, the agent's own judgement among them: one whose
/// text stands again in the policy's once the console's is taken away.
pub fn policy_holds(name: &str) -> bool {
  name == SUPERVISOR || DRAFTING.questions.contains_key(name)
}

/// The console's text of a supervisor's question taken away; whether there was one.
pub async fn remove_question<C: ConnectionTrait>(db: &C, agent: &str, name: &str) -> Result<bool, Fault> {
  let gone = run(
    db,
    r#"DELETE FROM "draft_questions" WHERE "agent" = $1 AND "name" = $2"#,
    vec![agent.into(), name.into()],
  )
  .await?;

  Ok(gone > 0)
}

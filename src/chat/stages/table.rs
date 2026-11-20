//! The instruction stage chain and its texts (docs/instruction.md): the policy's default chain and stage texts,
//! each supervisor's own chain and stage texts standing over them for that supervisor alone, the chain a round
//! reads and what a set may hold. The chain is an ordered list of stage names; each stage's text is the instruction
//! its turn carries, editable per supervisor. A supervisor with no chain of its own follows the policy's; an empty
//! chain runs no stage and the round's message goes out unchecked.

use std::collections::BTreeMap;

use receiver_policy::instruction::INSTRUCTION;
use sea_orm::{ConnectionTrait, QueryResult};
use serde_json::{Value, json};

use crate::store::Fault;
use crate::{all, now_ms, run};

/// A key that stands for no supervisor: the policy's own chain, which every supervisor follows until it sets one.
pub const POLICY: &str = "*";

/// The longest a stage's name may run.
pub const NAME_CHARS: usize = 40;

/// The longest a stage's text may run.
pub const TEXT_CHARS: usize = 4000;

/// A supervisor's chain of stages: the ordered names it follows once its message stands, and whether the console set
/// it. The policy's chain is the supervisor `*`, followed by every supervisor's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chain {
  pub agent: String,
  pub stages: Vec<String>,
  pub set: bool,
}

impl Chain {
  /// The policy's default chain, which a supervisor reads until the console sets a chain of its own.
  pub fn policy() -> Chain {
    Chain {
      agent: POLICY.to_owned(),
      stages: INSTRUCTION.chain.clone(),
      set: false,
    }
  }

  /// Whether the chain is a supervisor's own, the only kind the console sets: a supervisor's name, not the policy's.
  pub fn own(&self) -> bool {
    !self.agent.trim().is_empty() && self.agent != POLICY
  }

  /// The chain as the routes answer it.
  pub fn member(&self) -> Value {
    json!({ "agentName": self.agent, "stages": self.stages, "source": if self.set { "set" } else { "default" } })
  }
}

/// A stage of the catalogue: its name, its text and whether the console set the text. The policy's text of it, where
/// the policy holds the stage, stands again once the console's is taken away.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stage {
  pub name: String,
  pub text: String,
  pub set: bool,
}

impl Stage {
  /// The stage as the routes answer it, with the policy's text of it, null for a stage the console added.
  pub fn member(&self) -> Value {
    let policy_text = INSTRUCTION.stages.get(&self.name);

    json!({
      "name": self.name,
      "text": self.text,
      "source": if self.set { "set" } else { "default" },
      "policyText": policy_text,
    })
  }
}

fn names(text: &str) -> Vec<String> {
  serde_json::from_str(text).unwrap_or_default()
}

fn chain_row(row: &QueryResult) -> Result<Chain, Fault> {
  let text = |column: &str| row.try_get::<String>("", column);

  Ok(Chain {
    agent: text("agent")?,
    stages: names(&text("chain")?),
    set: true,
  })
}

impl Chain {
  /// The chain with each stage named once, in the order first named.
  pub fn dedup(mut self) -> Self {
    let mut seen: Vec<String> = vec![];

    for name in self.stages {
      if !seen.contains(&name) {
        seen.push(name.clone());
      }
    }

    self.stages = seen;
    self
  }
}

/// Every chain of the table: the policy's default, then each supervisor's own by the supervisor's name.
pub async fn chains<C: ConnectionTrait>(db: &C) -> Result<Vec<Chain>, Fault> {
  let read =
    all(db, r#"SELECT "agent", "chain" FROM "instruction_stages" ORDER BY "agent""#, vec![]).await?;
  let own = read
    .iter()
    .map(|row| chain_row(row).map(Chain::dedup))
    .collect::<Result<Vec<Chain>, Fault>>()?;

  Ok(std::iter::once(Chain::policy()).chain(own.into_iter().filter(Chain::own)).collect())
}

/// The chain a supervisor follows, out of every chain of the table: its own where the console set one, else the
/// policy's.
pub fn followed<'a>(rows: &'a [Chain], agent: &str) -> Option<&'a Chain> {
  rows
    .iter()
    .find(|row| row.own() && row.agent == agent)
    .or_else(|| rows.iter().find(|row| !row.own()))
}

/// The stage names a supervisor's round runs, read from the table; the policy's where the table cannot be read.
pub async fn chain<C: ConnectionTrait>(db: &C, agent: &str) -> Vec<String> {
  let rows = chains(db).await.unwrap_or_else(|_| vec![Chain::policy()]);

  followed(&rows, agent).map(|row| row.stages.clone()).unwrap_or_default()
}

/// A supervisor's own chain set by the console, over whatever stood at its name.
pub async fn set<C: ConnectionTrait>(db: &C, agent: &str, stages: &[String]) -> Result<(), Fault> {
  let listed = |list: &[String]| serde_json::to_string(list).unwrap_or_else(|_| "[]".to_owned());

  run(
    db,
    r#"INSERT INTO "instruction_stages" ("agent", "chain", "set_at") VALUES ($1, $2, $3)
       ON CONFLICT ("agent") DO UPDATE SET "chain" = EXCLUDED."chain", "set_at" = EXCLUDED."set_at""#,
    vec![agent.into(), listed(stages).into(), now_ms().into()],
  )
  .await
  .map(drop)
}

/// A supervisor's own chain taken away; whether there was one.
pub async fn remove_chain<C: ConnectionTrait>(db: &C, agent: &str) -> Result<bool, Fault> {
  let gone = run(db, r#"DELETE FROM "instruction_stages" WHERE "agent" = $1"#, vec![agent.into()]).await?;

  Ok(gone > 0)
}

/// Everything a supervisor has taken away with the supervisor: its chain and its stage texts.
pub async fn forget<C: ConnectionTrait>(db: &C, agent: &str) -> Result<(), Fault> {
  run(db, r#"DELETE FROM "instruction_stages" WHERE "agent" = $1"#, vec![agent.into()]).await?;
  run(db, r#"DELETE FROM "instruction_texts" WHERE "agent" = $1"#, vec![agent.into()]).await?;

  Ok(())
}

fn stands(name: &str, text: &str, set: &mut BTreeMap<String, String>) -> Stage {
  match set.remove(name) {
    Some(text) => Stage {
      name: name.to_owned(),
      text,
      set: true,
    },
    None => Stage {
      name: name.to_owned(),
      text: text.to_owned(),
      set: false,
    },
  }
}

/// A supervisor's stage catalogue: the policy's stages, each text replaced by the one the console set for that
/// supervisor, then the names the console added for it. With no supervisor named, the policy's alone.
pub async fn texts<C: ConnectionTrait>(db: &C, agent: Option<&str>) -> Result<Vec<Stage>, Fault> {
  let read = match agent {
    Some(agent) => {
      all(
        db,
        r#"SELECT "name", "text" FROM "instruction_texts" WHERE "agent" = $1 ORDER BY "name""#,
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

  let mut stages = INSTRUCTION
    .stages
    .iter()
    .map(|(name, text)| stands(name, text, &mut set))
    .collect::<Vec<Stage>>();

  stages.extend(set.into_iter().map(|(name, text)| Stage {
    name,
    text,
    set: true,
  }));

  Ok(stages)
}

/// A supervisor's own stage text set by the console for a stage.
pub async fn set_text<C: ConnectionTrait>(
  db: &C,
  agent: &str,
  name: &str,
  text: &str,
) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "instruction_texts" ("agent", "name", "text", "set_at") VALUES ($1, $2, $3, $4)
       ON CONFLICT ("agent", "name") DO UPDATE SET "text" = EXCLUDED."text", "set_at" = EXCLUDED."set_at""#,
    vec![agent.into(), name.into(), text.into(), now_ms().into()],
  )
  .await
  .map(drop)
}

/// The console's text of a supervisor's stage taken away; whether there was one.
pub async fn remove_text<C: ConnectionTrait>(db: &C, agent: &str, name: &str) -> Result<bool, Fault> {
  let gone = run(
    db,
    r#"DELETE FROM "instruction_texts" WHERE "agent" = $1 AND "name" = $2"#,
    vec![agent.into(), name.into()],
  )
  .await?;

  Ok(gone > 0)
}

/// Whether the policy holds a stage of the name, whose console text stands again in the policy's once taken away.
pub fn policy_holds(name: &str) -> bool {
  INSTRUCTION.stages.contains_key(name)
}

/// Why a chain of a supervisor may not be set, where it may not: every stage it names must have a text, out of the
/// policy or the supervisor's own set texts. The catalogue's names are the stages a chain may hold.
pub fn chain_refusal(stages: &[String], catalogue: &[String]) -> Option<String> {
  if let Some(unknown) = stages.iter().find(|name| !catalogue.contains(name)) {
    return Some(format!("The chain names the stage {unknown}, which the catalogue does not hold."));
  }

  None
}

/// Why a stage's name or text may not be set, where it may not.
pub fn text_refusal(name: &str, text: &str) -> Option<String> {
  let named = name.chars().count() <= NAME_CHARS
    && name.chars().next().is_some_and(|first| first.is_ascii_lowercase())
    && name.chars().all(|one| one.is_ascii_lowercase() || one.is_ascii_digit() || one == '-');

  if !named {
    return Some(format!(
      "A stage's name is a lowercase word of letters, digits and dashes opening on a letter, of at most \
       {NAME_CHARS} characters."
    ));
  }

  if reserved(name) {
    return Some(format!("The name {name} is the round's own turn's, and no stage takes it."));
  }

  if text.trim().is_empty() || text.chars().count() > TEXT_CHARS {
    return Some(format!("A stage's text is not empty and at most {TEXT_CHARS} characters."));
  }

  None
}

/// Whether a name is one a round's fixed turns go by in the round log, which no stage may take: the message turn's,
/// the turn's showing the words that arrived meanwhile, and the audit's.
pub fn reserved(name: &str) -> bool {
  [super::log::MESSAGE, super::log::ARRIVED, INSTRUCTION.audit.name.as_str()].contains(&name)
}

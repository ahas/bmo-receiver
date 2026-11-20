//! The audit of the people a message answers (docs/instruction.md, 「The audit before the stages」): what the agent is
//! asked to do for a person its session read nothing of, a person's sheet told by its path, and the people already
//! asked how they want to be called, kept per agent so none is asked twice.

use receiver_policy::instruction::Audit;
use receiver_ports::brain::Known;
use sea_orm::ConnectionTrait;

use crate::store::{Fault, settings};

/// What the audit asks of the agent for one person the session read nothing of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owed {
  /// Nothing: the brain holds nothing of them, and they were asked already.
  Nothing,
  /// Read their records, which hold a name for them or ask none any more.
  Read,
  /// Read their records and have the message ask how they want to be called: none of them is a name.
  ReadAndAsk,
  /// Have the message ask how they want to be called: the brain holds nothing of them.
  Ask,
}

impl Owed {
  /// The policy's line for the person, none where nothing is owed.
  pub fn line(self, said: &Audit) -> Option<&str> {
    match self {
      Self::Nothing => None,
      Self::Read => Some(&said.read),
      Self::ReadAndAsk => Some(&said.read_ask),
      Self::Ask => Some(&said.ask),
    }
  }

  /// Whether the line has the message ask the person how they want to be called.
  pub fn asks(self) -> bool {
    matches!(self, Self::ReadAndAsk | Self::Ask)
  }
}

/// What is owed for a person the session read nothing of, by what the brain holds of them and whether they were
/// asked how they want to be called before.
pub fn owed(known: Known, asked: bool) -> Owed {
  match (known.held, known.named || asked) {
    (true, true) => Owed::Read,
    (true, false) => Owed::ReadAndAsk,
    (false, false) => Owed::Ask,
    (false, true) => Owed::Nothing,
  }
}

/// The person a piece is the sheet of, by the key in its path (`…/people/<key>/…`); none for any other piece.
pub fn sheet_of(path: &str) -> Option<String> {
  let mut segments = path.split('/');

  segments.by_ref().find(|segment| *segment == "people")?;

  let key = segments.next()?;

  (!key.is_empty() && segments.next().is_some()).then(|| key.to_owned())
}

/// The setting the people an agent's audit asked how they want to be called stand under (`docs/store.md`).
pub fn asked_key(agent: &str) -> String {
  format!("{agent}_names_asked")
}

/// The people an agent's audit has asked how they want to be called, by key; none where the setting cannot be read.
pub async fn asked<C: ConnectionTrait>(db: &C, agent: &str) -> Vec<String> {
  settings::get(db, &asked_key(agent))
    .await
    .ok()
    .flatten()
    .and_then(|held| serde_json::from_str::<Vec<String>>(&held).ok())
    .unwrap_or_default()
}

/// The people an audit just asked, kept beside those asked before.
pub async fn keep_asked<C: ConnectionTrait>(db: &C, agent: &str, asking: &[String]) -> Result<(), Fault> {
  if asking.is_empty() {
    return Ok(());
  }

  let mut kept = asked(db, agent).await;

  for key in asking {
    if !kept.contains(key) {
      kept.push(key.clone());
    }
  }

  settings::set(db, &asked_key(agent), &serde_json::to_string(&kept).unwrap_or_default()).await
}

//! The instruction stages (docs/instruction.md): the chain of system instructions a message passes before it is
//! sent, the default chain and stage texts, and the brief rules the block carries for reading a message, an
//! instruction and answering each.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The audit of the people the message answers, asked before the chain where the session read nothing of one of
/// them: the name its row and marker go by, the text heading it and the line for each person it names.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audit {
  pub name: String,
  pub text: String,
  /// A person whose records the brain holds and the session has not read; `{name}` and `{id}` are theirs.
  pub read: String,
  /// A person the brain holds nothing about, never asked how they want to be called.
  pub ask: String,
  /// A person whose records the brain holds with no name among them, never asked.
  pub read_ask: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instruction {
  /// How a turn that carries a room's words is read, said once in the room's block.
  pub message_rule: String,
  /// How a turn marked INSTRUCTION, a system instruction, is read, said once in the block.
  pub instruction_rule: String,
  /// How an instruction is answered, said once in the block: an empty answer keeps the standing message.
  pub answer_rule: String,
  /// The line an instruction turn opens with, `{name}` its stage's name.
  pub marker: String,
  /// The line a message turn that skipped naming no reason is asked once more with.
  pub skip_again: String,
  /// The marker that names the instruction showing the words that arrived meanwhile.
  pub arrived_marker: String,
  /// What that instruction asks, said once in the room's block; `{voice}` is `voice`.
  pub arrived: String,
  /// The voice a message is written in: the persona's, never notes.
  pub voice: String,
  /// The heads an instruction turn shows its room's words, its rules and the people it answers under.
  pub words: String,
  pub rules: String,
  pub answers: String,
  /// One person the message answers, `{name}` as they go by in the room's words and `{id}` their sender number.
  pub person: String,
  /// A rule or knowledge this round carries that went in an earlier turn of the same turn: named by its path alone.
  pub handed: String,
  /// The head the instruction turn shows the room's words that went in full in an earlier turn of it under.
  pub room_handed: String,
  /// The audit of the people the message answers, asked before the chain.
  pub audit: Audit,
  /// The default chain of instruction stages, in the order they are asked once the message stands.
  pub chain: Vec<String>,
  /// The words of each stage's instruction, by the stage's name.
  pub stages: BTreeMap<String, String>,
}

pub static INSTRUCTION: std::sync::LazyLock<Instruction> = std::sync::LazyLock::new(|| {
  crate::read(
    "instruction.toml",
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy/instruction.toml")),
  )
});

//! Every part of the policy reads as its type, every file under `policy/` is a part, and the lines carry the marks
//! their readers fill (docs/policy.md).

use receiver_policy::answer_reading::ANSWER_READING;
use receiver_policy::chat_health::CHAT_HEALTH;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::commands::COMMANDS;
use receiver_policy::files::FILES;
use receiver_policy::fill_marks;
use receiver_policy::handover::HANDOVER;
use receiver_policy::instruction::INSTRUCTION;
use receiver_policy::keepalive::KEEPALIVE;
use receiver_policy::kept::KEPT;
use receiver_policy::open_files::OPEN_FILES;
use receiver_policy::owner_facts::OWNER_FACTS;
use receiver_policy::presence::PRESENCE;
use receiver_policy::providers::{Handover, PROVIDERS, Turns};
use receiver_policy::resident_turn::RESIDENT_TURN;
use receiver_policy::rest::REST;
use receiver_policy::restart::RESTART;
use receiver_policy::roads::ROADS;
use receiver_policy::roles::ROLES;
use receiver_policy::run_lines::RUN_LINES;
use receiver_policy::scripts::SCRIPTS;
use receiver_policy::secret_shapes::SECRET_SHAPES;
use receiver_policy::talk::TALK;
use receiver_policy::tool_host::TOOL_HOST;
use receiver_policy::voice_briefing::VOICE_BRIEFING;
use receiver_policy::wake::WAKE;

const POLICY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy");

/// Every part, by its file, read here so a file that does not read as its type fails this test.
fn parts() -> Vec<&'static str> {
  let _ = (&*CHAT_HEALTH, &*CHAT_LINE, &*CHAT_ROUND, &*COMMANDS);
  let _ = (&*FILES, &*HANDOVER, &*KEEPALIVE, &*PROVIDERS);
  let _ = (&*RESIDENT_TURN, &*REST, &*RESTART, &*ROADS, &*ROLES, &*RUN_LINES, &*SCRIPTS);
  let _ = (&*ANSWER_READING, &*KEPT, &*OPEN_FILES, &*INSTRUCTION, &*PRESENCE);
  let _ = (&*TALK, &*WAKE, &*OWNER_FACTS, &*TOOL_HOST, &*SECRET_SHAPES, &*VOICE_BRIEFING);

  vec![
    "answer-reading.toml",
    "chat-health.toml",
    "chat-line.toml",
    "chat-round.toml",
    "commands.toml",
    "files.toml",
    "fixed-lines.toml",
    "handover.toml",
    "instruction.toml",
    "keepalive.toml",
    "kept.toml",
    "open-files.toml",
    "owner-facts.toml",
    "presence.toml",
    "providers.toml",
    "resident-turn.toml",
    "rest.toml",
    "restart.toml",
    "roads.toml",
    "roles.toml",
    "run-lines.toml",
    "scripts.toml",
    "secret-shapes.toml",
    "talk.toml",
    "tool-host.toml",
    "voice-briefing.toml",
    "wake.toml",
  ]
}

#[test]
fn every_part_reads_and_every_file_is_a_part() {
  let mut files: Vec<String> = std::fs::read_dir(POLICY)
    .expect("the policy folder is readable")
    .map(|entry| entry.expect("a policy entry is readable").file_name().to_string_lossy().into_owned())
    .collect();
  files.sort();

  assert_eq!(files, parts());
}

#[test]
fn the_rounds_lines_carry_their_marks() {
  assert_eq!(CHAT_ROUND.recent_messages, 30);
  assert_eq!(
    CHAT_ROUND.archived_messages, 100,
    "the archived block carries 100 unread messages by default"
  );
  assert!(CHAT_ROUND.blocks.archived_left_out.contains("{count}"));
  assert!(CHAT_ROUND.shape.quoted.contains("{quote}"));
  assert!(CHAT_ROUND.closing.rule.contains("<<WAIT>>"));
  assert!(!CHAT_ROUND.index.rule.is_empty() && !CHAT_ROUND.queue_line.rule.is_empty());
  for word in ["quiet memory", "memory_done", "memory_update"] {
    assert!(CHAT_ROUND.index.memories.contains(word), "{word}");
  }
  assert!(CHAT_ROUND.index.memories.contains("{chat}"));
  assert_eq!(CHAT_ROUND.situation.gap_messages, 200);
  assert_eq!(CHAT_ROUND.voice.emotions.len(), 10);
}

#[test]
fn the_instruction_stages_are_a_default_chain_with_their_texts() {
  assert!(CHAT_ROUND.closing.rule.contains("<<SKIP: <why>>>"), "{}", CHAT_ROUND.closing.rule);
  // A skip of a round's turn names its reason, and the line that says so stands where the agent reads it: in the
  // room's block and in the judgement's question.
  for line in [
    &CHAT_LINE.room.process,
    &CHAT_ROUND.supervisor.question,
    &CHAT_ROUND.supervisor.structure,
  ] {
    assert!(line.contains("<<SKIP: <why>>>"), "{line}");
  }
  // The default chain's stages each have a text, and the chain is asked once the message stands.
  let chain = &INSTRUCTION.chain;

  assert!(
    chain.iter().all(|stage| INSTRUCTION.stages.contains_key(stage)),
    "every default chain stage has its text: {chain:?}"
  );
  assert!(!chain.is_empty());
  // The block's rules for reading a message, an instruction and answering a stage are each present and English.
  assert!(!INSTRUCTION.message_rule.is_empty() && !INSTRUCTION.instruction_rule.is_empty());
  let answer_rule = &INSTRUCTION.answer_rule;

  assert!(answer_rule.contains("{pass}"), "{answer_rule}");
  assert!(
    !fill_marks(answer_rule, &[("pass", &INSTRUCTION.pass)]).contains('{'),
    "every mark is filled"
  );
  assert!(
    INSTRUCTION.marker.contains("{name}") && !INSTRUCTION.standing.is_empty(),
    "an instruction names its stage"
  );
  // The room's block names the marks a message that arrived meanwhile takes, once.
  let process = &CHAT_LINE.room.process;

  assert!(
    process.contains("<<DONE #<number>>>") && process.contains("<<SEPARATE #<number>>>"),
    "{process}"
  );
}

#[test]
fn the_talks_name_the_owner_and_the_agent_by_their_marks() {
  assert_eq!(TALK.turns_given, 6);
  assert!(TALK.item.head[0].contains("{id}"));
  assert_eq!((TALK.owner.as_str(), TALK.agent.as_str()), ("{owner}", "{agent}"));
  assert!(CHAT_LINE.room.roster.contains("{agent}") && CHAT_LINE.room.roster.contains("{owner}"));
  assert_eq!(fill_marks(&TALK.says_now, &[("owner", "the-owner")]), "the-owner says now:");
}

#[test]
fn the_other_lines_carry_their_marks_and_numbers() {
  assert_eq!(FILES.unnamed, "attachment");
  assert!(RESTART.deployed.purpose.contains("{purpose}"));
  assert_eq!(RESTART.ride_minutes, 3);
  assert_eq!(WAKE.file_lead_seconds, 120);
  assert!(WAKE.file_lead_messages > 0);
  assert!(HANDOVER.transcript.answer.contains("{agent}"));
  assert!(CHAT_HEALTH.order.what.contains("{where}") && CHAT_HEALTH.order.check_seconds > 0);
  assert!(REST.lift_check_seconds > 0);
  assert!(RESIDENT_TURN.fold_ratio > 0.0 && RESIDENT_TURN.fold_ratio < 1.0);
  assert!(!KEEPALIVE.prompt.is_empty());
}

#[test]
fn the_roads_roles_lines_and_providers_answer_what_the_receiver_asks() {
  assert_eq!(ROADS.record.key, "record");
  assert!(!ROADS.marks.picture.is_empty() && !ROADS.marks.voice.is_empty());
  let filling = &ROADS.filling;
  assert!(!filling.heads.is_empty() && filling.heads.iter().all(|head| !head.trim().is_empty()));
  assert!(0 < filling.default_rows && filling.default_rows <= filling.most_rows);
  assert!(ROLES.supervisor_sends() && !ROLES.sends(&["graph.read", "queue.read"]));
  assert!(RUN_LINES.composes_typed_lines("chat") && !RUN_LINES.composes_typed_lines("worker"));
  let claude = PROVIDERS.provider("claude-code").expect("claude-code is declared");
  assert_eq!((claude.turns, claude.handover), (Turns::Transcript, Handover::Fold));
  assert_eq!(claude.keepalive_idle_minutes, Some(40));
  assert_eq!(claude.lifetime_minutes(), Some(60), "the touch is due before the cache expires");
  let registered = PROVIDERS.provider("registered").expect("registered is declared");
  assert_eq!((registered.turns, registered.handover), (Turns::Endpoint, Handover::Handoff));
  assert!(!registered.own_words.is_empty());
}

#[test]
fn every_command_has_its_plain_form_first() {
  let phrases = &COMMANDS.phrases;
  let firsts = [
    (&phrases.unmute, "member:unmute"),
    (&phrases.mute, "member:mute"),
    (&phrases.notice_restart_on, "notice:restart:on"),
    (&phrases.notice_restart_off, "notice:restart:off"),
    (&phrases.photos_on, "photos:on"),
    (&phrases.photos_off, "photos:off"),
  ];

  for (said, plain) in firsts {
    assert_eq!(said.first().map(String::as_str), Some(plain));
  }
  assert!(!COMMANDS.polite_tails.is_empty() && !COMMANDS.option_suffixes.is_empty());
}

#[test]
fn a_mark_is_filled_once_and_a_mark_nothing_names_stays() {
  assert_eq!(fill_marks("{a} and {b}", &[("a", "{b}"), ("b", "two")]), "{b} and two");
  assert_eq!(fill_marks("{missing} {", &[]), "{missing} {");
}

#[test]
fn an_answer_reading_table_that_still_names_a_language_loads_and_ignores_it() {
  use receiver_policy::answer_reading::AnswerReading;

  let policy =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy/answer-reading.toml"))
      .expect("the answer-reading policy reads");
  let text = format!("{policy}\n[language]\njudged_from = 4\n");
  let parsed: AnswerReading = toml::from_str(&text).expect("the old language keys load");

  assert_eq!(parsed.language.map(|language| language.judged_from), Some(4));
}

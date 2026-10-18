//! Every part of the policy reads as its type, every file under `policy/` is a part, and the lines carry the marks
//! their readers fill (docs/policy.md).

use receiver_policy::chat_health::CHAT_HEALTH;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::code_review::CODE_REVIEW;
use receiver_policy::commands::COMMANDS;
use receiver_policy::companion_runner::COMPANION_RUNNER;
use receiver_policy::companions::COMPANIONS;
use receiver_policy::consult::CONSULT;
use receiver_policy::files::FILES;
use receiver_policy::fill_marks;
use receiver_policy::handover::HANDOVER;
use receiver_policy::keepalive::KEEPALIVE;
use receiver_policy::owner_facts::OWNER_FACTS;
use receiver_policy::providers::{Handover, PROVIDERS, Turns};
use receiver_policy::queue::QUEUE;
use receiver_policy::resident_turn::RESIDENT_TURN;
use receiver_policy::rest::REST;
use receiver_policy::restart::RESTART;
use receiver_policy::roads::ROADS;
use receiver_policy::roles::ROLES;
use receiver_policy::run_lines::RUN_LINES;
use receiver_policy::second_asks::SECOND_ASKS;
use receiver_policy::talk::TALK;
use receiver_policy::wake::WAKE;

const POLICY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy");

/// Every part, by its file, read here so a file that does not read as its type fails this test.
fn parts() -> Vec<&'static str> {
  let _ = (&*CHAT_HEALTH, &*CHAT_LINE, &*CHAT_ROUND, &*CODE_REVIEW, &*COMMANDS, &*COMPANION_RUNNER);
  let _ = (&*COMPANIONS, &*CONSULT, &*FILES, &*HANDOVER, &*KEEPALIVE, &*PROVIDERS, &*QUEUE);
  let _ = (&*RESIDENT_TURN, &*REST, &*RESTART, &*ROADS, &*ROLES, &*RUN_LINES, &*SECOND_ASKS);
  let _ = (&*TALK, &*WAKE, &*OWNER_FACTS);

  vec![
    "chat-health.toml",
    "chat-line.toml",
    "chat-round.toml",
    "code-review.toml",
    "commands.toml",
    "companion-runner.toml",
    "companions.toml",
    "consult.toml",
    "files.toml",
    "handover.toml",
    "keepalive.toml",
    "owner-facts.toml",
    "providers.toml",
    "queue.toml",
    "resident-turn.toml",
    "rest.toml",
    "restart.toml",
    "roads.toml",
    "roles.toml",
    "run-lines.toml",
    "second-asks.toml",
    "talk.toml",
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
  assert!(CHAT_ROUND.shape.quoted.contains("{quote}"));
  assert!(CHAT_ROUND.closing.woken.contains("{language}"));
  assert!(CHAT_ROUND.closing.language_named.contains("{language}"));
  assert_eq!(CHAT_ROUND.voice.emotions.len(), 10);
  assert!(SECOND_ASKS.errands.owed.contains("{count}"));
  assert!(SECOND_ASKS.errands.wake_on.contains("{chat}"));
  assert!(SECOND_ASKS.errands.rule.contains("{chat}"));
  assert!(SECOND_ASKS.errands.wording.contains("{terms}"));
  assert_eq!(SECOND_ASKS.errands.language.matches("{what}").count(), 2);
  assert_eq!(SECOND_ASKS.reading.pledge_final, 8);
}

#[test]
fn the_talks_name_the_owner_the_agent_and_the_language_by_their_marks() {
  assert_eq!(TALK.turns_given, 6);
  assert!(TALK.item.head[0].contains("{id}"));
  assert_eq!((TALK.owner.as_str(), TALK.agent.as_str()), ("{owner}", "{agent}"));
  assert!(TALK.item.head.iter().any(|line| line.contains("{language}")));
  assert!(TALK.document.head.iter().any(|line| line.contains("{language}")));
  assert!(CODE_REVIEW.talk.head.iter().any(|line| line.contains("{owner}")));
  assert!(CHAT_LINE.room.roster.contains("{agent}") && CHAT_LINE.room.roster.contains("{owner}"));
  assert_eq!(fill_marks(&TALK.says_now, &[("owner", "the-owner")]), "the-owner says now:");
}

#[test]
fn the_other_lines_carry_their_marks_and_numbers() {
  assert!(CONSULT.work.item.contains("{title}"));
  assert!(CONSULT.windows.open.contains("{left}"));
  assert!(CONSULT.said.contains("{number}") && CONSULT.number.contains("{id}"));
  assert_eq!(FILES.largest_bytes, 50 * 1024 * 1024);
  assert!(RESTART.deployed.purpose.contains("{purpose}"));
  assert_eq!(RESTART.ride_minutes, 3);
  assert_eq!(WAKE.file_lead_seconds, 120);
  assert!(WAKE.file_lead_messages > 0);
  assert_eq!(QUEUE.question_stop_seconds, 30 * 60);
  assert!(HANDOVER.transcript.answer.contains("{agent}"));
  assert!(CHAT_HEALTH.failed.run_log.as_deref().is_some_and(|line| line.contains("{run}")));
  assert!(REST.threshold > 0.0 && REST.lift_check_seconds > 0);
  assert!(RESIDENT_TURN.fold_ratio > 0.0 && RESIDENT_TURN.fold_ratio < 1.0);
  assert!(COMPANION_RUNNER.limits.ms() > 0 && COMPANION_RUNNER.cut.instruction[0].contains("{reason}"));
  assert!(!KEEPALIVE.prompt.is_empty() && !COMPANIONS.name_pattern.is_empty());
}

#[test]
fn the_roads_roles_lines_and_providers_answer_what_the_receiver_asks() {
  assert_eq!(ROADS.record.key, "record");
  assert!(!ROADS.marks.picture.is_empty() && !ROADS.marks.voice.is_empty());
  assert!(ROLES.sends("chat") && !ROLES.sends("worker"));
  assert!(RUN_LINES.composes_typed_lines("chat") && !RUN_LINES.composes_typed_lines("worker"));
  let claude = PROVIDERS.provider("claude-code").expect("claude-code is declared");
  assert_eq!((claude.turns, claude.handover), (Turns::Transcript, Handover::Fold));
  assert_eq!(claude.keepalive_idle_minutes, Some(40));
  let registered = PROVIDERS.provider("registered").expect("registered is declared");
  assert_eq!((registered.turns, registered.handover), (Turns::Endpoint, Handover::Handoff));
  assert!(!registered.own_words.is_empty());
}

#[test]
fn every_command_has_its_plain_form_first() {
  let phrases = &COMMANDS.phrases;
  let firsts = [
    (&phrases.delegate_on, "delegate:on"),
    (&phrases.delegate_off, "delegate:off"),
    (&phrases.unmute, "member:unmute"),
    (&phrases.mute, "member:mute"),
    (&phrases.notice_rest_on, "notice:rest:on"),
    (&phrases.notice_rest_off, "notice:rest:off"),
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

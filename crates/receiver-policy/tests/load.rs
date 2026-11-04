//! Every part of the policy reads as its type, every file under `policy/` is a part, and the lines carry the marks
//! their readers fill (docs/policy.md).

use receiver_policy::chat_health::CHAT_HEALTH;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::commands::COMMANDS;
use receiver_policy::consult::CONSULT;
use receiver_policy::drafting::DRAFTING;
use receiver_policy::files::FILES;
use receiver_policy::fill_marks;
use receiver_policy::handover::HANDOVER;
use receiver_policy::keepalive::KEEPALIVE;
use receiver_policy::kept::KEPT;
use receiver_policy::owner_facts::OWNER_FACTS;
use receiver_policy::providers::{Handover, PROVIDERS, Turns};
use receiver_policy::quiet::QUIET;
use receiver_policy::resident_turn::RESIDENT_TURN;
use receiver_policy::rest::REST;
use receiver_policy::restart::RESTART;
use receiver_policy::roads::ROADS;
use receiver_policy::roles::ROLES;
use receiver_policy::run_lines::RUN_LINES;
use receiver_policy::scripts::SCRIPTS;
use receiver_policy::second_asks::SECOND_ASKS;
use receiver_policy::secret_shapes::SECRET_SHAPES;
use receiver_policy::talk::TALK;
use receiver_policy::tool_host::TOOL_HOST;
use receiver_policy::wake::WAKE;

const POLICY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy");

/// Every part, by its file, read here so a file that does not read as its type fails this test.
fn parts() -> Vec<&'static str> {
  let _ = (&*CHAT_HEALTH, &*CHAT_LINE, &*CHAT_ROUND, &*COMMANDS);
  let _ = (&*CONSULT, &*FILES, &*HANDOVER, &*KEEPALIVE, &*PROVIDERS);
  let _ = (&*RESIDENT_TURN, &*REST, &*RESTART, &*ROADS, &*ROLES, &*RUN_LINES, &*SCRIPTS);
  let _ = (&*SECOND_ASKS, &*KEPT, &*DRAFTING, &*QUIET);
  let _ = (&*TALK, &*WAKE, &*OWNER_FACTS, &*TOOL_HOST, &*SECRET_SHAPES);

  vec![
    "chat-health.toml",
    "chat-line.toml",
    "chat-round.toml",
    "commands.toml",
    "consult.toml",
    "drafting.toml",
    "files.toml",
    "fixed-lines.toml",
    "handover.toml",
    "keepalive.toml",
    "kept.toml",
    "owner-facts.toml",
    "providers.toml",
    "quiet.toml",
    "resident-turn.toml",
    "rest.toml",
    "restart.toml",
    "roads.toml",
    "roles.toml",
    "run-lines.toml",
    "scripts.toml",
    "second-asks.toml",
    "secret-shapes.toml",
    "talk.toml",
    "tool-host.toml",
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
  assert!(CHAT_ROUND.closing.woken.contains("{reason}"));
  assert_eq!(CHAT_ROUND.situation.gap_messages, 200);
  assert_eq!(CHAT_ROUND.voice.emotions.len(), 10);
  assert!(SECOND_ASKS.errands.owed.contains("{count}"));
  assert!(SECOND_ASKS.errands.wake_on.contains("{chat}"));
  assert!(SECOND_ASKS.errands.rule.contains("{chat}"));
  assert_eq!(SECOND_ASKS.reading.pledge_final, 8);
  assert!(!SECOND_ASKS.errands.format.is_empty());
}

#[test]
fn the_draft_process_ends_every_list_on_its_last_question() {
  assert!(DRAFTING.lead.contains("{number}") && DRAFTING.lead.contains("{count}"));
  assert!(DRAFTING.opening.contains("{number}") && DRAFTING.opening.contains("{send}"));
  assert!(DRAFTING.lead.contains("{send}") && DRAFTING.send_open.contains("<<SEND>>"));
  assert!(CHAT_ROUND.closing.questioned_owed.contains("{count}"));
  assert!(DRAFTING.last_lead.contains("{number}") && DRAFTING.waiting.contains("{ids}"));
  assert!(DRAFTING.questions.contains_key("final"));
  // Every line whose answer may be the message carries the room's voice; the closed send line, a notes one, does not.
  for line in [&DRAFTING.send_open, &DRAFTING.last_lead, &DRAFTING.arrived] {
    assert!(line.contains("{voice}"), "{line}");
  }
  assert!(!DRAFTING.send_closed.contains("{voice}") && !DRAFTING.voice.is_empty());

  // The room's block names the marks a message that arrived meanwhile takes, once for every question, and so does
  // the closing check; the lead lines name only a question's place.
  for line in [&CHAT_LINE.room.process, &DRAFTING.arrived] {
    assert!(line.contains("<<DONE #<number>>>") && line.contains("<<SEPARATE #<number>>>"), "{line}");
  }

  for process in &DRAFTING.process {
    for list in [&process.rounds, &process.light] {
      assert!(list.iter().all(|name| DRAFTING.questions.contains_key(name)), "{process:?}");
      assert_eq!(list.last().map(String::as_str), Some("final"), "{process:?}");
    }

    assert!(
      process.send_from.is_empty() || process.rounds.contains(&process.send_from),
      "{process:?}"
    );
  }

  assert!(DRAFTING.process.iter().any(|process| process.provider == "*" && process.agent == "*"));
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
  assert!(CONSULT.work.item.contains("{title}"));
  assert!(CONSULT.windows.open.contains("{left}"));
  assert!(CONSULT.said.contains("{number}") && CONSULT.number.contains("{id}"));
  assert_eq!(FILES.largest_bytes, 50 * 1024 * 1024);
  assert!(RESTART.deployed.purpose.contains("{purpose}"));
  assert_eq!(RESTART.ride_minutes, 3);
  assert_eq!(WAKE.file_lead_seconds, 120);
  assert!(WAKE.file_lead_messages > 0);
  assert!(HANDOVER.transcript.answer.contains("{agent}"));
  assert!(CHAT_HEALTH.order.what.contains("{where}") && CHAT_HEALTH.order.check_seconds > 0);
  assert!(REST.threshold > 0.0 && REST.lift_check_seconds > 0);
  assert!(RESIDENT_TURN.fold_ratio > 0.0 && RESIDENT_TURN.fold_ratio < 1.0);
  assert!(!KEEPALIVE.prompt.is_empty());
}

#[test]
fn the_roads_roles_lines_and_providers_answer_what_the_receiver_asks() {
  assert_eq!(ROADS.record.key, "record");
  assert!(!ROADS.marks.picture.is_empty() && !ROADS.marks.voice.is_empty());
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
fn a_process_row_that_still_names_a_ceiling_loads_and_ignores_it() {
  let policy = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy/drafting.toml"))
    .expect("the drafting policy reads");
  let text = format!(
    "{policy}\n[[process]]\nprovider = \"*\"\ntier = \"*\"\nagent = \"*\"\nrounds = [\"final\"]\nsend_from = \"\"\nlight = [\"final\"]\nlight_chars = 12\nceiling_seconds = 150\npast_ceiling = \"send\"\n"
  );
  let parsed: receiver_policy::drafting::Drafting = toml::from_str(&text).expect("the old keys load");

  let row = parsed.process.last().expect("the appended row stands");

  assert_eq!(row.ceiling_seconds, Some(150));
  assert_eq!(row.past_ceiling.as_deref(), Some("send"));
}

#[test]
fn the_policy_has_one_process_for_every_agent_and_the_old_two_collapse_to_it() {
  use receiver_policy::drafting::{Drafting, Process};

  let five = vec!["information", "facts", "memory", "promises", "final"];

  assert_eq!(DRAFTING.process.len(), 1);
  assert_eq!(DRAFTING.process[0].provider, "*");
  assert_eq!(DRAFTING.process[0].tier, "*");
  assert_eq!(DRAFTING.process[0].agent, "*");
  assert_eq!(DRAFTING.process[0].rounds, five);
  assert!(DRAFTING.questions.contains_key("intent") && !five.contains(&"intent"));

  let policy = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy/drafting.toml"))
    .expect("the drafting policy reads");
  let mut old: Drafting = toml::from_str(&policy).expect("the policy reads");

  old.process = vec![
    Process {
      provider: "*".to_owned(),
      tier: "*".to_owned(),
      agent: "*".to_owned(),
      rounds: vec!["information".to_owned(), "facts".to_owned(), "final".to_owned()],
      send_from: "facts".to_owned(),
      light: vec!["final".to_owned()],
      light_chars: 12,
      ceiling_seconds: Some(150),
      past_ceiling: Some("send".to_owned()),
      turn_seconds: None,
    },
    Process {
      provider: "claude-code".to_owned(),
      tier: "*".to_owned(),
      agent: "*".to_owned(),
      rounds: five.iter().map(|name| (*name).to_owned()).collect(),
      send_from: "facts".to_owned(),
      light: vec!["final".to_owned()],
      light_chars: 12,
      ceiling_seconds: Some(150),
      past_ceiling: Some("send".to_owned()),
      turn_seconds: None,
    },
  ];
  old.normalize();

  assert_eq!(old.process.len(), 1);
  assert_eq!(old.process[0].provider, "*");
  assert_eq!(old.process[0].tier, "*");
  assert_eq!(old.process[0].agent, "*");
  assert_eq!(
    old.process[0].rounds,
    five.iter().map(|name| (*name).to_owned()).collect::<Vec<String>>()
  );
}

#[test]
fn a_second_asks_table_that_still_names_a_language_loads_and_ignores_it() {
  use receiver_policy::second_asks::SecondAsks;

  let policy =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy/second-asks.toml"))
      .expect("the second-asks policy reads");
  let with_errand = policy.replace("[errands]\n", "[errands]\nlanguage = \"old language errand\"\n");
  let text = format!("{with_errand}\n[language]\njudged_from = 4\n");
  let parsed: SecondAsks = toml::from_str(&text).expect("the old language keys load");

  assert_eq!(parsed.errands.language.as_deref(), Some("old language errand"));
  assert_eq!(parsed.language.map(|language| language.judged_from), Some(4));
}

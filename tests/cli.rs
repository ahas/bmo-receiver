//! The commands on the binary's own (docs/boundary-api.md, 「Configuration」): what `--help` lists, every flat
//! name read as the command it names, and the route table the `routes` command prints — the same names the
//! router answers, each once.

use std::process::{Command, Output};

use bmo_receiver::routes::served;

// The commands the binary declares, as its subcommands' names come to.
const COMMANDS: [&str; 15] = [
  "serve",
  "routes",
  "deliver-waiting",
  "tool-env",
  "health",
  "nouns",
  "import",
  "room",
  "photo",
  "react",
  "pin",
  "sticker",
  "reminder",
  "consult",
  "draw",
];

// What the help lists: the commands, and the built-in `help` beside them.
const LISTED: [&str; 16] = [
  "serve",
  "routes",
  "deliver-waiting",
  "tool-env",
  "health",
  "nouns",
  "import",
  "room",
  "photo",
  "react",
  "pin",
  "sticker",
  "reminder",
  "consult",
  "draw",
  "help",
];

// The nouns `bmo` dispatches to this command, each on the operations machine.
const NOUNS: [&str; 8] = [
  "room say\thome",
  "photo\thome",
  "react\thome",
  "pin\thome",
  "sticker\thome",
  "reminder set\thome",
  "consult\thome",
  "draw\thome",
];

// The binary run with `words`, from wherever the test stands.
fn run(words: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_bmo-receiver")).args(words).output().unwrap()
}

fn said(output: &Output) -> String {
  let out = String::from_utf8_lossy(&output.stdout).into_owned();
  let err = String::from_utf8_lossy(&output.stderr).into_owned();

  out + err.as_str()
}

// The command names the help lists, in the order it lists them.
fn listed(help: &Output) -> Vec<String> {
  said(help)
    .lines()
    .skip_while(|line| !line.trim_start().starts_with("Commands:"))
    .skip(1)
    .take_while(|line| line.starts_with("  "))
    .map(|line| line.split_whitespace().next().unwrap_or_default().to_owned())
    .collect::<Vec<String>>()
}

#[test]
fn the_help_lists_every_command_and_none_of_two_words() {
  let help = run(&["--help"]);
  let words = said(&help);

  assert!(help.status.success(), "the help was refused: {words}");
  let names = listed(&help);
  let wanted = LISTED.into_iter().map(|name| name.to_owned()).collect::<Vec<String>>();

  assert_eq!(names, wanted, "the help lists {names:?} instead of {wanted:?}");
  assert!(
    names.iter().all(|name| !name.contains(' ')),
    "a command of two words is listed: {names:?}"
  );
}

#[test]
fn every_flat_name_reads_as_its_command() {
  for command in COMMANDS {
    let asked = run(&[command, "--help"]);
    let words = said(&asked);

    assert!(asked.status.success(), "{command} is not a command: {words}");
    assert!(words.contains(command), "{command} reads as another command: {words}");
  }

  let unknown = run(&["no-such-command"]);
  let words = said(&unknown);

  assert_eq!(unknown.status.code(), Some(2), "an unknown command is taken: {words}");
}

#[test]
fn the_nouns_bmo_dispatches_are_declared_one_a_line() {
  let printed = run(&["nouns"]);
  let words = said(&printed);

  assert!(printed.status.success(), "the nouns were refused: {words}");

  let lines: Vec<String> = String::from_utf8_lossy(&printed.stdout).lines().map(str::to_owned).collect();
  let wanted: Vec<String> = NOUNS.iter().map(|noun| (*noun).to_owned()).collect();

  assert_eq!(lines, wanted, "the command declares {lines:?} instead of {wanted:?}");
}

#[test]
fn every_noun_reads_as_the_command_bmo_dispatches() {
  let nouns: [&[&str]; 8] = [
    &["room", "say"],
    &["photo"],
    &["react"],
    &["pin"],
    &["sticker"],
    &["reminder", "set"],
    &["consult"],
    &["draw"],
  ];

  for noun in nouns {
    let mut asked = noun.to_vec();

    asked.push("--help");
    let help = run(&asked);
    let words = said(&help);

    assert!(help.status.success(), "{noun:?} is not a command: {words}");
    assert!(words.contains(noun[noun.len() - 1]), "{noun:?} reads as another command: {words}");
  }
}

#[test]
fn the_routes_command_prints_the_table_once_each() {
  let printed = run(&["routes"]);
  let words = said(&printed);

  assert!(printed.status.success(), "the routes were refused: {words}");
  let names = String::from_utf8_lossy(&printed.stdout)
    .into_owned()
    .lines()
    .map(|line| line.to_owned())
    .collect::<Vec<String>>();
  let table = served();

  assert_eq!(names, table, "the printed routes are not the table");

  let unique = names.iter().map(|name| name.to_owned()).collect::<std::collections::BTreeSet<String>>();

  assert_eq!(unique.len(), names.len(), "a route is printed twice: {names:?}");
}

#[test]
fn a_configuration_that_does_not_read_ends_the_ask_with_2() {
  // No configuration stands in a test run's own directory, so the service cannot be reached.
  let health = run(&["health"]);
  let words = said(&health);

  assert_eq!(health.status.code(), Some(2), "the health ask ended as it stands: {words}");
  assert!(
    words.contains("the configuration could not be read"),
    "the health ask says otherwise: {words}"
  );
}

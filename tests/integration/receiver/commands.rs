//! The owner's setting commands read off the words typed (docs/rooms.md): a phrase of the policy's with the agent's
//! name in front of it and a polite ending after it, a number only after a mute, and the mode its own word.

use bmo_receiver::receiver::commands::{CommandKind, OwnerCommand, parse_owner_command};
use receiver_policy::commands::COMMANDS;

fn addresses() -> Vec<String> {
  vec!["agent-a".to_owned(), "agent".to_owned()]
}

fn read(text: &str) -> Option<CommandKind> {
  parse_owner_command(text, &addresses()).map(|command| command.kind)
}

fn last(phrases: &[String]) -> &str {
  phrases.last().unwrap()
}

#[test]
fn a_phrase_is_read_with_the_agents_name_before_it_and_a_polite_ending_after_it() {
  let phrases = &COMMANDS.phrases;
  let tails = &COMMANDS.polite_tails;

  assert_eq!(
    read(&format!("agent-a, {}{}", last(&phrases.photos_on), tails[4])),
    Some(CommandKind::PhotosOn)
  );
  assert_eq!(
    read(&format!("agent {}{}!", last(&phrases.unmute), tails[4])),
    Some(CommandKind::Unmute)
  );
  assert_eq!(read("notice:restart:off"), Some(CommandKind::NoticeRestartOff));
  assert_eq!(read(&format!("{}.", last(&phrases.photos_off))), Some(CommandKind::PhotosOff));
  assert_eq!(read(&format!("{} story{}", last(&phrases.photos_on), tails[0])), None);
}

#[test]
fn the_longest_name_the_agent_is_called_by_is_left_off_first() {
  // Read with the shorter name alone, "agent-a" would leave "-a" in front of the phrase.
  assert_eq!(read("agent-a photos:on"), Some(CommandKind::PhotosOn));
  assert_eq!(parse_owner_command("agent-a photos:on", &["agent".to_owned()]), None);
}

#[test]
fn only_a_mute_names_a_person_by_number() {
  let mute = last(&COMMANDS.phrases.mute);

  assert_eq!(
    parse_owner_command(&format!("{mute} 12345"), &addresses()),
    Some(OwnerCommand {
      kind: CommandKind::Mute,
      explicit_target_id: Some(12345),
    })
  );
  assert_eq!(parse_owner_command("photos:on 12345", &addresses()), None);
  assert_eq!(parse_owner_command(mute, &addresses()).unwrap().explicit_target_id, None);
}

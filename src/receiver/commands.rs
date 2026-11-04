//! The owner's setting commands, read off the words the owner typed (docs/rooms.md): a phrase of the policy's,
//! with the agent's name in front of it and a polite ending after it left out, and for a mute the person's number
//! after it. The phrases and the endings are the policy's (`policy/commands.toml`); the words a room calls the
//! agent by are the deployment's.

use receiver_policy::commands::COMMANDS;

/// The settings the owner switches by saying so in a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
  Allow,
  Deny,
  Mute,
  Unmute,
  NoticeRestartOn,
  NoticeRestartOff,
  PhotosOn,
  PhotosOff,
}

/// One command, and the person it names by number where it names one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerCommand {
  pub kind: CommandKind,
  pub explicit_target_id: Option<i64>,
}

// Each command with the phrases that say it. A kind the policy gives no phrase is never said in words.
fn phrases() -> [(CommandKind, &'static [String]); 6] {
  let said = &COMMANDS.phrases;

  [
    (CommandKind::Unmute, &said.unmute),
    (CommandKind::Mute, &said.mute),
    (CommandKind::NoticeRestartOn, &said.notice_restart_on),
    (CommandKind::NoticeRestartOff, &said.notice_restart_off),
    (CommandKind::PhotosOn, &said.photos_on),
    (CommandKind::PhotosOff, &said.photos_off),
  ]
}

/// The words with a name the agent is called by left off their front, the longest name first, and every polite
/// ending taken off their back, their spacing made single.
pub fn normalize(text: &str, addresses: &[String]) -> String {
  let mut rest = text.trim();
  let mut names: Vec<&String> = addresses.iter().filter(|name| !name.is_empty()).collect();

  names.sort_by_key(|name| std::cmp::Reverse(name.len()));

  if let Some(name) = names.iter().find(|name| rest.starts_with(name.as_str())) {
    rest = rest[name.len()..].trim_start_matches(|c: char| c.is_whitespace() || c == ',');
  }

  let mut rest = rest.to_owned();
  let mut stripped = true;

  while stripped {
    stripped = false;
    rest = rest.trim_end().to_owned();

    if let Some(tail) = COMMANDS.polite_tails.iter().find(|tail| rest.ends_with(tail.as_str())) {
      rest.truncate(rest.len() - tail.len());
      stripped = true;
    }
  }

  rest.split_whitespace().collect::<Vec<_>>().join(" ")
}

// The phrase and the number after it, where the words end on one: a space and digits alone.
fn phrase_and_number(normalized: &str) -> (&str, Option<i64>) {
  let Some((phrase, digits)) = normalized.rsplit_once(' ') else {
    return (normalized, None);
  };

  if phrase.is_empty() || digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
    return (normalized, None);
  }

  match digits.parse() {
    Ok(number) => (phrase, Some(number)),
    Err(_) => (normalized, None),
  }
}

/// The setting command the words are, or none, the agent called by any of the names given.
pub fn parse_owner_command(text: &str, addresses: &[String]) -> Option<OwnerCommand> {
  let normalized = normalize(text, addresses);
  let (phrase, explicit_target_id) = phrase_and_number(&normalized);
  let (kind, _) = phrases().into_iter().find(|(_, phrases)| phrases.iter().any(|said| said == phrase))?;

  if explicit_target_id.is_some() && !matches!(kind, CommandKind::Mute | CommandKind::Unmute) {
    return None;
  }

  Some(OwnerCommand {
    kind,
    explicit_target_id,
  })
}

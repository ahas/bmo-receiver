//! The receiver's policy (docs/policy.md): its fixed numbers and words, one TOML file to a part under the
//! repository's `policy/`, compiled in here. Each part is read once, on first use, into its type.

use serde::de::DeserializeOwned;

pub mod chat_health;
pub mod chat_line;
pub mod chat_round;
pub mod commands;
pub mod drafting;
pub mod files;
pub mod fixed_lines;
pub mod handover;
pub mod keepalive;
pub mod kept;
pub mod open_files;
pub mod owner_facts;
pub mod providers;
pub mod resident_turn;
pub mod rest;
pub mod restart;
pub mod roads;
pub mod roles;
pub mod run_lines;
pub mod scripts;
pub mod second_asks;
pub mod secret_shapes;
pub mod talk;
pub mod tool_host;
pub mod voice_briefing;
pub mod wake;

/// A line of the policy with each `{name}` it holds filled in, every one at once so a value that itself holds a
/// mark is left as it is; a mark nothing names stays. The marks are the policy's own syntax, a name of letters,
/// digits and underscores between braces, which no general template format reads as such.
pub fn fill_marks(line: &str, values: &[(&str, &str)]) -> String {
  let mut out = String::new();
  let mut rest = line;

  while let Some(open) = rest.find('{') {
    out.push_str(&rest[..open]);

    let after = &rest[open + 1..];
    let name_end = after
      .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
      .unwrap_or(after.len());
    let value = (name_end > 0 && after[name_end..].starts_with('}'))
      .then(|| values.iter().find(|(name, _)| *name == &after[..name_end]))
      .flatten();

    match value {
      Some((_, value)) => {
        out.push_str(value);
        rest = &after[name_end + 1..];
      }
      None => {
        out.push('{');
        rest = after;
      }
    }
  }

  out + rest
}

/// One part read from its file's text. A file that does not read as its type is a fault of the build, which the
/// crate's load test meets before anything runs on it.
pub fn read<T: DeserializeOwned>(file: &str, text: &str) -> T {
  toml::from_str(text)
    .unwrap_or_else(|error| panic!("the receiver's policy's {file} does not read: {error}"))
}

/// A part of the policy, compiled in from its file under the repository's `policy/` and read once on first use:
/// `crate::part!(pub WAKE: Wake = "wake.toml");`
macro_rules! part {
  ($vis:vis $name:ident: $type:ty = $file:literal) => {
    $vis static $name: std::sync::LazyLock<$type> = std::sync::LazyLock::new(|| {
      $crate::read($file, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy/", $file)))
    });
  };
}

pub(crate) use part;

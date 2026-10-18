//! The procedures as a room may use them (docs/chat-mcp.md, 「One list in every room」): the documents under the
//! chat's procedures folder of the tree a reader stands in, each named by its file, and one read with the
//! sections and lines for a verb the room's road does not perform left out. The verbs a road performs are the
//! running receiver's to say, since only it holds the roads; the receiver's procedures tool asks it for them,
//! and the `bmo procedure show` command reads them where it stands. Both read the documents and hold them to
//! the verbs here, named as the chat protocol names them.

use std::path::{Path, PathBuf};

use bmo_chat_client::ChatCapability;
use regex::Regex;
use serde_json::{Value, json};

use crate::bot::Service;

/// The route the verbs a room's road performs are asked of.
pub const CAN_PATH: &str = "/sends/can";

/// The folder the procedure documents stand in, in the tree a reader stands in.
pub fn procedures_dir(tree: &Path) -> PathBuf {
  tree.join("prompts").join("agents").join("chat").join("procedures")
}

/// Every procedure by the name a session names it, which is its file's own without the extension.
pub fn procedure_names(tree: &Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(procedures_dir(tree))
    .map(|entries| {
      entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_string_lossy().strip_suffix(".md").map(str::to_owned))
        .collect()
    })
    .unwrap_or_default();

  names.sort();
  names
}

/// A procedure document as it is written, or why it cannot be read: a name that is no procedure is refused
/// with the names there are rather than read.
pub fn written(tree: &Path, name: &str) -> Result<String, String> {
  let names = procedure_names(tree);

  if !names.iter().any(|one| one == name) {
    return Err(format!("no procedure named \"{name}\"; there are: {}", names.join(", ")));
  }

  std::fs::read_to_string(procedures_dir(tree).join(format!("{name}.md")))
    .map_err(|why| format!("the procedure {name} could not be read: {why}"))
}

/// A procedure document's text as a room whose road performs `can` may use it, or whole where it is read
/// outside any room; its trailing blank lines dropped either way.
pub fn as_used(text: &str, can: Option<&[String]>) -> Result<String, String> {
  let used = match can {
    Some(can) => filter_procedure(text, can)?,
    None => text.to_owned(),
  };

  Ok(used.trim_end().to_owned())
}

/// The verbs the road a room is answered on performs, `via` naming the road where it is not the room's own,
/// as the running receiver answers them.
pub async fn road_can(receiver: &Service, chat: i64, via: Option<&str>) -> Result<Vec<String>, String> {
  let said = receiver.post(CAN_PATH, &json!({ "chat": chat, "via": via })).await?;

  said
    .get("can")
    .and_then(Value::as_array)
    .map(|verbs| verbs.iter().filter_map(Value::as_str).map(str::to_owned).collect())
    .ok_or_else(|| format!("the receiver answered {CAN_PATH} with no verbs"))
}

// The mark, and the words around it that are never printed: ` [CAP:sticker]`.
fn capability_mark() -> Regex {
  Regex::new(r"\s*\[CAP:([A-Za-z,]+)\]\s*$").expect("the capability mark is a pattern")
}

// The verbs one line's mark names. A mark naming something that is no capability is a broken document.
fn mark_of(mark: &Regex, line: &str) -> Result<Vec<String>, String> {
  let Some(found) = mark.captures(line) else { return Ok(Vec::new()) };

  found[1]
    .split(',')
    .map(|word| {
      if named_capabilities().iter().any(|one| one == word) {
        Ok(word.to_owned())
      } else {
        Err(format!("a procedure marks \"{word}\", which is no capability"))
      }
    })
    .collect()
}

// Every capability, in the words the chat protocol names it on the wire.
fn named_capabilities() -> Vec<String> {
  ChatCapability::ALL
    .iter()
    .filter_map(|capability| {
      serde_json::to_value(capability).ok().and_then(|said| said.as_str().map(str::to_owned))
    })
    .collect::<Vec<_>>()
}

fn indent_of(line: &str) -> usize {
  line.len() - line.trim_start().len()
}

// Where the block that starts at `start` ends: a heading's whole section, or a line and everything indented
// under it.
fn block_end(lines: &[&str], start: usize, limit: usize) -> usize {
  if lines[start].starts_with("## ") {
    return lines
      .iter()
      .enumerate()
      .skip(start + 1)
      .find(|(_, line)| line.starts_with("## "))
      .map_or(limit, |(next, _)| next.min(limit));
  }

  let indent = indent_of(lines[start]);
  let mut end = start + 1;

  while end < limit && !lines[end].trim().is_empty() && indent_of(lines[end]) > indent {
    end += 1;
  }

  end
}

// Where a block's own lines end: a heading owns its line alone; any other block its line and the lines it
// wraps onto, down to the first item written under it.
fn own_end(lines: &[&str], start: usize, limit: usize) -> usize {
  if lines[start].starts_with("## ") {
    return start + 1;
  }

  lines
    .iter()
    .enumerate()
    .skip(start + 1)
    .find(|(_, line)| line.trim_start().starts_with("- "))
    .map_or(limit, |(item, _)| item.min(limit))
}

fn filter_range(
  mark: &Regex,
  lines: &[&str],
  from: usize,
  to: usize,
  can: &[String],
) -> Result<Vec<String>, String> {
  let mut kept = Vec::new();
  let mut index = from;

  while index < to {
    let line = lines[index];

    if line.trim().is_empty() {
      kept.push(line.to_owned());
      index += 1;
      continue;
    }

    let end = block_end(lines, index, to);
    let own = own_end(lines, index, end);
    let mut marks = Vec::new();

    for one in &lines[index..own] {
      marks.extend(mark_of(mark, one)?);
    }

    if marks.iter().all(|verb| can.contains(verb)) {
      kept.extend(lines[index..own].iter().map(|one| mark.replace(one, "").into_owned()));
      kept.extend(filter_range(mark, lines, own, end, can)?);
    }

    index = end;
  }

  Ok(kept)
}

/// The document as a room whose road performs `can` reads it: the marked parts it cannot use gone, and the
/// marks themselves gone from what is left.
pub fn filter_procedure(text: &str, can: &[String]) -> Result<String, String> {
  let mark = capability_mark();
  let lines: Vec<&str> = text.split('\n').collect();

  Ok(format!("{}\n", filter_range(&mark, &lines, 0, lines.len(), can)?.join("\n").trim_end()))
}

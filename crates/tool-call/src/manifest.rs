//! A tool's manifest held to its executable (docs/chat-mcp.md, 「How a call runs」): every entry `tool.toml`
//! declares answered by the executable, with the description, hints, capabilities and grant it answers under,
//! and the schema files it naming holding the schemas its input and output types make.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde_json::Value;

use crate::handed::PROTOCOL;

/// A schema a type makes.
pub type MadeSchema = fn() -> Value;

/// One entry as the executable answers it.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
  pub namespace: &'static str,
  pub name: &'static str,
  pub title: Option<&'static str>,
  pub description: &'static str,
  /// The schema file of its input, and the schema its input type makes.
  pub input: &'static str,
  pub input_schema: MadeSchema,
  /// The schema file of its output, where it declares one, and the schema that output is.
  pub output: Option<(&'static str, MadeSchema)>,
  pub capabilities: &'static [&'static str],
  /// The hints it declares, each by its name.
  pub hints: &'static [(&'static str, bool)],
  pub grant: Option<&'static str>,
}

impl Entry {
  /// The entry's full name, as a call names it.
  pub fn full_name(&self) -> String {
    format!("{}/{}", self.namespace, self.name)
  }
}

/// The schema a type makes as an input, as the protocol wants it: the object's schema alone, without the draft it
/// was written to or the Rust type's own name.
pub fn input_schema<T: JsonSchema>() -> Value {
  let mut schema = serde_json::to_value(schemars::schema_for!(T)).unwrap_or(Value::Null);

  if let Value::Object(fields) = &mut schema {
    fields.remove("$schema");
    fields.remove("title");
  }

  schema
}

fn schema_file(dir: &Path, file: &str) -> Result<Value, String> {
  let text = std::fs::read_to_string(dir.join(file)).map_err(|why| format!("its schema {file}: {why}"))?;

  serde_json::from_str(&text).map_err(|why| format!("its schema {file}: {why}"))
}

fn text<'a>(table: &'a toml::Table, key: &str) -> Option<&'a str> {
  table.get(key).and_then(toml::Value::as_str)
}

/// Where the manifest in `dir` and the entries the executable answers differ, one sentence each; none where they
/// agree.
pub fn differences(dir: &Path, entries: &[Entry]) -> Vec<String> {
  let manifest: toml::Table = match std::fs::read_to_string(dir.join("tool.toml")).map(|read| read.parse())
  {
    Ok(Ok(table)) => table,
    Ok(Err(why)) => return vec![format!("tool.toml does not read: {why}")],
    Err(why) => return vec![format!("tool.toml: {why}")],
  };
  let mut differ = vec![];
  let protocol = manifest
    .get("tool")
    .and_then(|tool| tool.get("protocol"))
    .and_then(toml::Value::as_integer);

  if protocol != Some(i64::from(PROTOCOL)) {
    differ.push(format!("tool.toml speaks protocol {protocol:?}, and the executable {PROTOCOL}"));
  }

  let declared: Vec<&toml::Table> = manifest
    .get("entry")
    .and_then(toml::Value::as_array)
    .map(|list| list.iter().filter_map(toml::Value::as_table).collect())
    .unwrap_or_default();

  for table in &declared {
    let at = format!(
      "{}/{}",
      text(table, "namespace").unwrap_or_default(),
      text(table, "name").unwrap_or_default(),
    );
    let Some(entry) = entries.iter().find(|entry| entry.full_name() == at) else {
      differ.push(format!("{at}: no entry of the executable answers it"));
      continue;
    };
    let capabilities: BTreeSet<&str> = table
      .get("capabilities")
      .and_then(toml::Value::as_array)
      .map(|list| list.iter().filter_map(toml::Value::as_str).collect())
      .unwrap_or_default();
    let hints: BTreeMap<String, bool> = table
      .get("hints")
      .and_then(toml::Value::as_table)
      .map(|hints| hints.iter().filter_map(|(name, on)| Some((name.clone(), on.as_bool()?))).collect())
      .unwrap_or_default();
    let output = text(table, "output");
    let said = [
      ("title", text(table, "title") == entry.title),
      ("description", text(table, "description") == Some(entry.description)),
      ("input file", text(table, "input") == Some(entry.input)),
      ("output file", output == entry.output.map(|(file, _)| file)),
      ("capabilities", capabilities == entry.capabilities.iter().copied().collect()),
      (
        "hints",
        hints == entry.hints.iter().map(|(name, on)| ((*name).to_owned(), *on)).collect(),
      ),
      ("grant", text(table, "grant") == entry.grant),
    ];

    for (what, same) in said {
      if !same {
        differ.push(format!("{at}: its {what} differs from the executable's"));
      }
    }

    match schema_file(dir, entry.input) {
      Ok(schema) if schema == (entry.input_schema)() => {}
      Ok(_) => differ.push(format!(
        "{at}: {} is not its input type's schema, which is {}",
        entry.input,
        (entry.input_schema)()
      )),
      Err(why) => differ.push(format!("{at}: {why}")),
    }

    if let Some((file, made)) = entry.output {
      match schema_file(dir, file) {
        Ok(schema) if schema == made() => {}
        Ok(_) => differ.push(format!("{at}: {file} is not its output's schema")),
        Err(why) => differ.push(format!("{at}: {why}")),
      }
    }
  }

  for entry in entries {
    let at = entry.full_name();

    if !declared.iter().any(|table| {
      format!(
        "{}/{}",
        text(table, "namespace").unwrap_or_default(),
        text(table, "name").unwrap_or_default(),
      ) == at
    }) {
      differ.push(format!("{at}: no entry of tool.toml declares it"));
    }
  }

  differ
}

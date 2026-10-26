//! The parts of a request's session record the receiver's tools read, and the turn's variables read afresh from
//! it (docs/chat-mcp.md, 「Turn context»). The record is the launcher's; a field it holds that no tool of this
//! repository reads is passed over.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

/// One session's record, its field names camel case as the launcher writes them.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
  /// The data root the run reads.
  #[serde(default)]
  pub data_root: PathBuf,
  /// A resident program's place, where each request reads which run is going.
  #[serde(default)]
  pub place: Option<PathBuf>,
  /// The run's own variables, for a run spawned for itself.
  #[serde(default)]
  pub run_variables: BTreeMap<String, String>,
  /// The issuer's labels, where the tool host's record carries the place.
  #[serde(default)]
  pub labels: serde_json::Map<String, Value>,
}

/// The variable a call's data root is read from where its record names none.
pub const DATA_ROOT_VARIABLE: &str = "BMO_DATA_DIR";

impl Session {
  /// The record as the tool reads it (docs/chat-mcp.md, 「How a call runs」): its data root, where it names none,
  /// read from `data_root`, the call's `BMO_DATA_DIR`; its place, where it names none, from its `labels.place`.
  pub fn completed(mut self, data_root: Option<String>) -> Self {
    if self.data_root.as_os_str().is_empty()
      && let Some(root) = data_root.filter(|root| !root.trim().is_empty())
    {
      self.data_root = PathBuf::from(root.trim());
    }

    if self.place.is_none()
      && let Some(place) =
        self.labels.get("place").and_then(Value::as_str).filter(|place| !place.is_empty())
    {
      self.place = Some(PathBuf::from(place));
    }

    self
  }

  /// Every variable of the turn the request is made in: the run the resident program's place names standing over
  /// the run's own variables, and a person's turn naming none from the place.
  pub fn turn_variables(&self) -> BTreeMap<String, String> {
    let mut variables = self.run_variables.clone();

    variables.extend(place_variables(self));
    variables
  }
}

// The place's run variables, none where it names a person's turn or holds no run.
fn place_variables(session: &Session) -> BTreeMap<String, String> {
  let from_place = session.place.as_ref().and_then(|place| -> Option<BTreeMap<String, String>> {
    let read = std::fs::read_to_string(place.join("run.json")).ok()?;
    let run: Value = serde_json::from_str(&read).ok()?;

    if run.get("person").and_then(Value::as_bool) == Some(true) {
      return Some(BTreeMap::new());
    }

    Some(
      run
        .get("env")
        .and_then(Value::as_object)
        .map(|env| {
          env
            .iter()
            .filter_map(|(name, value)| value.as_str().map(|value| (name.clone(), value.to_owned())))
            .collect()
        })
        .unwrap_or_default(),
    )
  });

  from_place.unwrap_or_default()
}

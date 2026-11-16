//! The running receiver as a placed tool and a shell's command reach it (docs/chat-mcp.md, 「Where each unit is
//! found」): what the receiver's env file under the data root says of its address, its token and the deployment
//! it answers for, since a placed tool's process holds none of them, a variable this process does holding its
//! place. One call to that address for what only the running receiver does, given a bound of its own: a call the
//! receiver has not answered within it ends with why, never waits on.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use dotenvy::from_path_iter;
use reqwest::Client;
use reqwest::header::CONTENT_TYPE;

/// The env file under the data root the receiver's `tool-env` verb writes as it is deployed.
pub const ENV_FILE: &str = "env/receiver.env";

/// The variable naming the receiver's base address.
pub const ADDRESS_VARIABLE: &str = "RECEIVER_ADDRESS";

/// The variable naming the bearer token the receiver takes.
pub const TOKEN_VARIABLE: &str = "RECEIVER_TOKEN";

/// The variable naming the owner, whose number the receiver answers for.
pub const OWNER_VARIABLE: &str = "BMO_OWNER_ID";

/// The variable naming the tree the receiver's own documents and scripts stand in.
pub const SOURCE_VARIABLE: &str = "BMO_SOURCE_DIR";

/// How long a connection to the receiver is given to open: it answers on this machine, so one that has not
/// opened by then is not coming.
const CONNECT_WITHIN: Duration = Duration::from_secs(5);

/// The receiver a data root belongs to: where it answers and the token it takes, the owner it answers for,
/// and the tree its documents are read from, where that is named.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Service {
  pub address: String,
  pub token: Option<String>,
  pub owner: Option<i64>,
  pub source: Option<PathBuf>,
}

/// The tree a placed tool stands in, whose documents and scripts the receiver itself reads: the nearest folder
/// above where the call runs holding them, as the `bmo` command finds the tree it stands in; else the tree the
/// env file names.
pub fn placed_tree(data_root: &Path) -> PathBuf {
  std::env::current_dir()
    .ok()
    .and_then(|here| {
      here
        .ancestors()
        .find(|tree| tree.join("prompts").is_dir() && tree.join("scripts").is_dir())
        .map(|tree| tree.to_path_buf())
    })
    .or_else(|| Service::of(data_root).source)
    .unwrap_or_default()
}

impl Service {
  /// The receiver the data root's env file describes, each variable this process holds standing first. A data
  /// root with no such file names what this process holds alone.
  pub fn of(data_root: &Path) -> Self {
    let file = settings(data_root);
    let named = |name: &str| {
      std::env::var(name)
        .ok()
        .or_else(|| file.get(name).cloned())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    };

    Self {
      address: named(ADDRESS_VARIABLE).unwrap_or_default(),
      token: named(TOKEN_VARIABLE),
      owner: named(OWNER_VARIABLE).and_then(|owner| owner.parse().ok()),
      source: named(SOURCE_VARIABLE).map(PathBuf::from),
    }
  }

  /// One `POST` to the receiver under its token, answered within `within`, answering the JSON the route answered
  /// with, or why the receiver did not do it: no token, nothing answering, no answer in time, or the route's own
  /// refusal.
  pub async fn post(&self, path: &str, body: &Value, within: Duration) -> Result<Value, String> {
    let (status, said) = self.post_answered(path, body, within).await?;

    if (200..300).contains(&status) {
      return Ok(said);
    }

    let why = said.get("error").and_then(Value::as_str).unwrap_or("no reason given");

    Err(format!("the receiver refused {path} ({status}): {why}"))
  }

  /// One `POST` to the receiver under its token, waited on until the receiver answers, the connection fails or
  /// `within` has passed: the status and the JSON the route answered with, whatever the status, or why nothing
  /// answered.
  pub async fn post_answered(
    &self,
    path: &str,
    body: &Value,
    within: Duration,
  ) -> Result<(u16, Value), String> {
    let Some(token) = &self.token else {
      return Err(format!(
        "{TOKEN_VARIABLE} is set nowhere this call reads, so the receiver cannot be asked"
      ));
    };
    let unanswered = |why: reqwest::Error| {
      if why.is_timeout() {
        format!(
          "the receiver did not answer {path} within {} seconds, so this call stopped waiting; what it asked may \
           still be done, so look before asking it again",
          within.as_millis().div_ceil(1000)
        )
      } else {
        format!("the receiver did not answer {path}: {why}")
      }
    };
    let client = Client::builder()
      .no_proxy()
      .connect_timeout(CONNECT_WITHIN.min(within))
      .timeout(within)
      .build()
      .map_err(|why| format!("no client for the receiver: {why}"))?;
    let written = serde_json::to_string(body).unwrap_or_default();
    let answer = client
      .post(format!("{}{}", self.address.trim(), path))
      .bearer_auth(token)
      .header(CONTENT_TYPE, "application/json")
      .body(written)
      .send()
      .await
      .map_err(unanswered)?;
    let status = answer.status().as_u16();
    let bytes = answer.bytes().await.map_err(unanswered)?;

    Ok((status, serde_json::from_slice(&bytes).unwrap_or(Value::Null)))
  }
}

// Every variable an env file under the data root names, as the service's own process is given them; none where
// there is no such file.
pub fn env_settings(file: &Path) -> HashMap<String, String> {
  from_path_iter(file)
    .map(|pairs| pairs.filter_map(Result::ok).collect())
    .unwrap_or_default()
}

// Every variable the receiver's env file names.
fn settings(data_root: &Path) -> HashMap<String, String> {
  env_settings(&data_root.join(ENV_FILE))
}

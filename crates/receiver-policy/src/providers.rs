//! What each harness provider declares, as far as the receiver reads it to compose a conversation's turns
//! (docs/policy.md): how a turn goes in and is read back, how a full session is handed over, when a touch is due,
//! and the words its program writes that are its own.

use std::collections::BTreeMap;

use serde::Deserialize;

/// How a conversation's full session is handed over: the program's own fold put into it as a turn, or the session
/// asked in one more turn for a note a new session opens with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Handover {
  #[default]
  Fold,
  Handoff,
}

/// How a turn of a standing program goes in and is read back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Turns {
  Transcript,
  Endpoint,
  Rollout,
  Loop,
}

/// What one provider declares.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
  pub turns: Turns,
  #[serde(default)]
  pub handover: Handover,
  /// How long a session stands idle before the touch is due on it, in minutes, where the provider keeps a cache.
  #[serde(default)]
  pub keepalive_idle_minutes: Option<u64>,
  /// How long its cache keeps a session's prefix, in minutes, where it declares one: a session idle longer is
  /// expired, neither touched nor woken, and replaced by its next round.
  #[serde(default)]
  pub cache_minutes: Option<u64>,
  /// The words its program writes into a turn that are its own, never the agent's.
  #[serde(default)]
  pub own_words: Vec<String>,
}

/// Every provider, by its key.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct Providers(pub BTreeMap<String, Provider>);

impl Provider {
  /// How long a session of it stays warm: its cache lifetime, else the idle stretch its touch is due after.
  pub fn lifetime_minutes(&self) -> Option<u64> {
    self.cache_minutes.or(self.keepalive_idle_minutes)
  }
}

impl Providers {
  pub fn provider(&self, key: &str) -> Option<&Provider> {
    self.0.get(key)
  }
}

crate::part!(pub PROVIDERS: Providers = "providers.toml");

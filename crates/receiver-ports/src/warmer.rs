//! The cache warmer's port: the receiver registered as a consumer with its hook, its chat touch registered as a
//! target, and each use of the target reported (`docs/boundary-api.md`, 「The cache warmer」;
//! the warmer's `docs/api.md`). The bodies are the warmer's published shapes: camelCase keys, epoch milliseconds,
//! and the bearer token the configuration holds for it, as every route takes.

use futures_util::future::BoxFuture;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::http::{Answer, Caller, Peer};

/// A target as the consumer registers or restates it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
  pub kind: String,
  pub label: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub subject_name: Option<String>,
  pub rank: i64,
  pub idle_ms: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub ttl_ms: Option<i64>,
  pub margin_ms: i64,
  pub ping_bound_ms: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub group_name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub warm_by_default: Option<bool>,
  pub failure_touches: bool,
  pub honours_idle_override: bool,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub used_at: Option<i64>,
}

/// A use of a target's prefix: when it was last written and when the work that used it began.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Used {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub at: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub started_at: Option<i64>,
}

/// Every call the receiver makes on the cache warmer.
pub trait Warmer: Send + Sync {
  /// The consumer registered, or its hook moved.
  fn register<'a>(&'a self, consumer: &'a str, url: &'a str) -> BoxFuture<'a, Answer<()>>;

  /// A target registered or restated.
  fn target<'a>(
    &'a self,
    consumer: &'a str,
    target: &'a str,
    declared: &'a Target,
  ) -> BoxFuture<'a, Answer<()>>;

  /// A use of the target's prefix reported.
  fn used<'a>(&'a self, consumer: &'a str, target: &'a str, used: Used) -> BoxFuture<'a, Answer<()>>;
}

/// The cache warmer over HTTP.
#[derive(Debug, Clone)]
pub struct HttpWarmer {
  caller: Caller,
}

impl HttpWarmer {
  pub fn new(peer: &Peer) -> Self {
    Self::with_caller(Caller::new(peer))
  }

  pub fn with_caller(caller: Caller) -> Self {
    Self { caller }
  }
}

#[derive(Serialize)]
struct Hook<'a> {
  url: &'a str,
}

impl Warmer for HttpWarmer {
  fn register<'a>(&'a self, consumer: &'a str, url: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move { self.caller.done(Method::PUT, &["consumers", consumer], &Hook { url }).await })
  }

  fn target<'a>(
    &'a self,
    consumer: &'a str,
    target: &'a str,
    declared: &'a Target,
  ) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      self
        .caller
        .done(Method::PUT, &["consumers", consumer, "targets", target], declared)
        .await
    })
  }

  fn used<'a>(&'a self, consumer: &'a str, target: &'a str, used: Used) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      self
        .caller
        .done(Method::POST, &["consumers", consumer, "targets", target, "uses"], &used)
        .await
    })
  }
}

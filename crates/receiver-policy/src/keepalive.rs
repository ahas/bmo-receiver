//! The cache warmer's touch of the chat agent's session, as the receiver puts it in (docs/bmo-window.md, 「The
//! touch」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keepalive {
  pub prompt: String,
}

crate::part!(pub KEEPALIVE: Keepalive = "keepalive.toml");

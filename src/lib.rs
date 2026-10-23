//! The chat receiver, `bmo-receiver` (docs/boundary-api.md): what becomes of a message whichever chat service
//! hands it over. It keeps the rooms, their rules, rights and windows, builds the envelope a turn goes out in,
//! and wakes the agents a room's words, a quiet instruction or a job's end call for. Every other unit is
//! reached through a port over HTTP; the chat services through bmo-chat-client.

pub mod chat;
pub mod command;
pub mod companion;
pub mod config;
pub mod dashes;
pub mod data_root;
pub mod events;
pub mod form;
pub mod hearing;
pub mod import;
pub mod json;
pub mod kept;
pub mod receiver;
pub mod roads;
pub mod room_record;
pub mod rounds;
pub mod routes;
pub mod run_lines;
pub mod service_start;
pub mod store;
pub mod text;
pub mod tool_env;
pub mod verbs;
pub mod web_address;
pub mod wording;
pub mod workers;

/// The text readings a JavaScript side is kept answers with, held in the shared crate so the receiver's verbs
/// and its tools read it alike.
pub use room_calls::js;

pub use store::{Fault, all, one, run};

/// The service's version, the workspace's: a contract a consumer reads moves the major, an addition no
/// existing consumer reads the minor, anything else the patch.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The moment it is, in epoch milliseconds.
pub fn now_ms() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
}

//! The chat receiver, `bmo-receiver` (docs/boundary-api.md): what becomes of a message whichever chat service
//! hands it over. It keeps the rooms, their rules, rights and windows, builds the envelope a turn goes out in,
//! and wakes the agents a room's words, a quiet instruction or a job's end call for. Every other unit is
//! reached through a port over HTTP; the chat services through bmo-chat-client.

pub mod chat;
pub mod companion;
pub mod dashes;
pub mod data_root;
pub mod form;
pub mod js;
pub mod json;
pub mod receiver;
pub mod roads;
pub mod room_record;
pub mod rounds;
pub mod run_lines;
pub mod store;
pub mod text;
pub mod web_address;
pub mod wording;

pub use store::{Fault, all, one, run};

/// The moment it is, in epoch milliseconds.
pub fn now_ms() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
}

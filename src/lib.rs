//! The chat receiver, `bmo-receiver` (docs/boundary-api.md): what becomes of a message whichever chat service
//! hands it over. It keeps the rooms, their rules, rights and windows, builds the envelope a turn goes out in,
//! and wakes the agents a room's words, a quiet instruction or a job's end call for. Every other unit is
//! reached through a port over HTTP; the chat services through bmo-chat-client.

pub mod data_root;
pub mod store;

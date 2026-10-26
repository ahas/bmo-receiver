//! The receiver's ports to the other units, one module to a unit (`docs/boundary-api.md`, 「Calls the receiver
//! makes」). Each is a trait the receiver's code is written against and an HTTP client that answers it, made from
//! the unit's base address and token as the configuration gives them; a test answers a trait with a fake of its
//! own, or the client with a stand-in of the unit's routes.

pub mod agency;
pub mod chat;
pub mod gate;
pub mod http;
pub mod job;
pub mod tool_host;
pub mod voice;
pub mod warmer;
pub mod web;

pub use http::{Answer, Caller, Patience, Peer, PortFailure, Wait, WireError};

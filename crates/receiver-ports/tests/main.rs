//! Each port's client tried against a stand-in of its unit: every call made on the method, path and body the
//! receiver's `docs/boundary-api.md` gives it, and its answer read as the call's shape. The crate's one test
//! harness (bmo-chat's docs/conventions.md, 「Rust builds」), the stand-in a module the others share.

mod stand_in;

mod agency;
mod brain;
mod chat;
mod gate;
mod holders;
mod http;
mod job;
mod tool_host;
mod warmer;
mod web;

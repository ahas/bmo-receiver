//! Each port's client tried against a stand-in of its unit: every call made on the method, path and body the
//! receiver's `docs/boundary-api.md` gives it, and its answer read as the call's shape. One binary, so the
//! stand-in and the clients are built once.

mod stand_in;

mod agency;
mod brain;
mod chat;
mod gate;
mod http;
mod job;
mod voice;
mod warmer;
mod web;

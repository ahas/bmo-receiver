//! The chat receiver's fixed API (`docs/api.md`, `docs/openapi.json`): every body and member as a Rust type, one
//! call for each route, and the OpenAPI document generated from those types. The TypeScript package beside this
//! crate is generated from the same document.
//!
//! The wire format is bmo-chat's (`docs/conventions.md`): camelCase keys, times as integers of epoch milliseconds,
//! ids as signed 64-bit integers, lists as pages of items and more, and every failure as a non-2xx answer carrying
//! `code` and `error`.

mod client;
mod openapi;
mod types;
mod wire;

pub use client::{ReceiverClient, wire_error};
pub use openapi::{KeepMarks, PromiseAsk, ReadMarkCounts, RoomSince, openapi, openapi_json};
pub use types::*;
pub use wire::{Answer, ClientFailure, Page, WireError};

/// The service's version, the workspace's.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

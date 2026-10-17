//! What the receiver's integration tests stand on: a test run alone in a process of its own, and a receiver
//! database of a test's own, migrated by the store's own migrator.

#![allow(dead_code)]

pub mod alone;
pub mod scratch;

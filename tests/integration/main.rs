//! The receiver's integration tests, one module to an area, built as one test binary: each binary links
//! every dependency whole, so one of them rather than one per area is what a build makes and keeps. The
//! modules share the process; a test that sets what stands for the whole of it runs alone in a process of
//! its own (`common::alone`).

mod common;

mod alone_runs;
mod data_root;
mod scratch_runs;
mod store;

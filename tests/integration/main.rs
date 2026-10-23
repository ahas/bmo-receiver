//! The receiver's integration tests, one module to an area, built as one test binary: each binary links
//! every dependency whole, so one of them rather than one per area is what a build makes and keeps. The
//! modules share the process; a test that sets what stands for the whole of it runs alone in a process of
//! its own (`common::alone`).

mod common;

mod agency_events;
mod alone_runs;
mod brain_contract;
mod companion_round;
mod companions;
mod consult_block;
mod consult_verbs;
mod contracts;
mod data_root;
mod draft_process;
mod import;
mod kept;
mod private_thread;
mod receiver;
mod relaunch;
mod room_record;
mod room_removal;
mod room_round;
mod routes;
mod scratch_runs;
mod session_loads;
mod stop_notice;
mod store;
mod talks;
mod verbs_draws;
mod verbs_messages;
mod wake;
mod window_close;
mod working_turn_push;

//! The receiver's integration tests, one module to an area, and what they stand on in `common`, which the
//! harness's other modules use as well. The modules share the process; a test that sets what stands for the
//! whole of it runs alone in a process of its own (`common::alone`).

pub(crate) mod common;

mod agency_events;
mod agent_rounds;
mod alone_runs;
mod avatars;
mod brain_contract;
mod briefing;
mod contracts;
mod cut_agency;
mod data_root;
mod double_slots;
mod draft_process;
mod expiry;
mod handed_sends;
mod handoffs;
mod handovers;
mod import;
mod kept;
mod message_acts;
mod notes_owed;
mod owed_replies;
mod owner_room;
mod private_thread;
mod reattach;
mod receiver;
mod relaunch;
mod reset_point;
mod room_record;
mod room_removal;
mod room_round;
mod routes;
mod scratch_runs;
mod session_generation;
mod session_loads;
mod session_replacement;
mod situation;
mod spoken_talk;
mod stop_notice;
mod store;
mod supervisor;
mod supervisor_routes;
mod talks;
mod thinking;
mod tools_heard;
mod verbs_draws;
mod verbs_messages;
mod wake;
mod window_close;
mod working_turn_push;

//! The repository's one test harness (bmo-chat's docs/conventions.md, 「Rust builds」): the checks that span the
//! repository and the service's own tests a module per file, the integration tests a module of their own. Every
//! module shares the process; a test that sets what stands for the whole of it runs alone in a process of its own
//! (`integration::common::alone`).

mod archived;
mod ask;
mod backlog;
mod cli;
mod config;
mod context;
mod conversation;
mod dashes;
mod draft;
mod draws;
mod events;
mod fixed_lines;
mod form;
mod integration;
mod js;
mod mid_turn;
mod openings;
mod parity;
mod personal_data;
mod progress;
mod promises;
mod quiet;
mod records;
mod relay;
mod repository;
mod room;
mod room_files;
mod secrets;
mod shape;
mod talk;
mod text;
mod tool_env;
mod turn;
mod web_address;
mod window;

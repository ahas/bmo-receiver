//! The conversations of every conversational agent in its rooms (docs/wake.md; docs/bmo-window.md): each room's
//! queue, rounds, window and waits, the round's turn on the agent's session with the words pushed into it, the
//! answer sent on the road the turn answers on, and the instruction pass over every agent's rows.

pub mod around;
pub mod context;
pub mod conversation;
pub mod instructions;
pub mod mid_turn;
pub mod prompt;
pub mod records;
pub mod room;
pub mod runner;
pub mod sentinels;
pub mod shape;
pub mod thread;
pub mod typed;
pub mod window;

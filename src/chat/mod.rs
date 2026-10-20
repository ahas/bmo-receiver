//! The conversations of every conversational agent in its rooms (docs/wake.md; docs/bmo-window.md): each room's
//! queue, rounds, window and waits, the round's turn on the agent's session with the words pushed into it, the
//! answer sent on the road the turn answers on, and the instruction pass over every agent's rows.

pub mod again;
pub mod around;
pub mod ask;
pub mod cache;
pub mod context;
pub mod conversation;
pub mod fallback;
pub mod fold;
pub mod health;
pub mod instructions;
pub mod logins;
pub mod mid_turn;
pub mod notices;
pub mod overtime;
pub mod progress;
pub use room_calls::promises;
pub mod prompt;
pub mod records;
pub mod recovered;
pub mod reply;
pub mod restart;
pub mod room;
pub mod runner;
pub mod sentinels;
pub mod shape;
pub mod talk;
pub mod thread;
pub mod touch;
pub mod turnover;
pub mod typed;
pub mod window;

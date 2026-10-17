//! The receiver's database (`docs/store.md`): its migrations, the pool and the statement helpers every read and
//! write goes through, and the settings rows.

pub mod migration;
pub mod pool;
pub mod settings;

pub use pool::{Fault, all, one, run};

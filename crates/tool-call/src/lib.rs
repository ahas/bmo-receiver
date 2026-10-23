//! The receiver's side of the tool host's call protocol (docs/chat-mcp.md, 「How a call runs」), which every tool
//! bound to the receiver shares: the one request a placed executable is handed on standard input and the one
//! object it answers on standard output, the parts of the call's session record the tools read and the turn they
//! read from it, what a call is answered with, where the units a tool answers through are found from the record's
//! data root, and the manifest a tool's tests hold to its executable.

mod answer;
mod handed;
mod manifest;
pub mod peers;
mod session;

pub use answer::{Answer, Content};
pub use handed::{Asked, Handed, PROTOCOL, answer_standard_io, read_handed};
pub use manifest::{Entry, MadeSchema, differences, input_schema};
pub use peers::{AGENCY_ENV_FILE, AGENCY_PORT, AGENCY_PORT_VARIABLE, AGENCY_TOKEN_VARIABLE, agency};
pub use session::Session;

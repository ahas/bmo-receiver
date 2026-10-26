//! The one request a placed executable is handed on standard input and the one object it answers on standard
//! output (docs/chat-mcp.md, 「How a call runs」).

use std::future::Future;
use std::process::ExitCode;

use serde::Deserialize;
use serde_json::Value;

use crate::session::Session;

/// The protocol version the receiver's tools speak.
pub const PROTOCOL: u32 = 1;

/// What a placed executable is asked.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Asked {
  /// One entry, by its full name, called with its arguments; answered with a tool result.
  Call {
    entry: String,
    #[serde(default)]
    arguments: Value,
  },
  /// The resources it serves, answered `{"resources": [...]}`.
  ResourcesList,
  /// One resource, answered `{"error": "<why>"}`, since the receiver's tools serve none.
  ResourcesRead { uri: String },
  /// A kind no tool of this repository answers.
  #[serde(other)]
  Other,
}

/// Everything the executable is handed: the protocol version, what it is asked and the record it is asked under.
/// The sockets the host hands beside them are passed over, since no tool of this repository calls back.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Handed {
  pub protocol: u32,
  #[serde(flatten)]
  pub asked: Asked,
  pub session: Session,
}

/// The request read off `input`, or why it does not read: an object that is no request, or one in a protocol
/// version the receiver's tools do not speak.
pub fn read_handed(input: impl std::io::Read) -> Result<Handed, String> {
  let mut handed: Handed =
    serde_json::from_reader(input).map_err(|why| format!("the request does not read: {why}"))?;

  handed.session = handed.session.completed(std::env::var(crate::session::DATA_ROOT_VARIABLE).ok());

  if handed.protocol != PROTOCOL {
    return Err(format!(
      "the request speaks protocol {}, and this tool speaks {PROTOCOL}",
      handed.protocol
    ));
  }

  Ok(handed)
}

/// A placed executable's whole `main`: its request read off standard input, answered by `answer`, and the answer
/// written on standard output. A request that does not read, or one `answer` refuses, is said on standard error
/// and ends the executable unclean, which the host takes as a failed call.
pub async fn answer_standard_io<F, Answered>(answer: F) -> ExitCode
where
  F: FnOnce(Handed) -> Answered,
  Answered: Future<Output = Result<Value, String>>,
{
  let said = match read_handed(std::io::stdin().lock()) {
    Ok(handed) => answer(handed).await,
    Err(why) => Err(why),
  };
  let written = said.and_then(|said| {
    serde_json::to_writer(std::io::stdout().lock(), &said)
      .map_err(|why| format!("the answer could not be written: {why}"))
  });

  match written {
    Ok(()) => ExitCode::SUCCESS,
    Err(why) => {
      eprintln!("{why}");
      ExitCode::FAILURE
    }
  }
}

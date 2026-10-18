//! The message a call of a tool that acts acts on, which the tool host checks the grant of
//! (docs/chat-mcp.md, 「Turn context」), whichever folder the tool stands in.

use serde::{Deserialize, Serialize};

/// The message a call acts on; a tool that names its source message already reads that one as this.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActedOn(pub i64);

impl schemars::JsonSchema for ActedOn {
  fn inline_schema() -> bool {
    true
  }

  fn schema_name() -> std::borrow::Cow<'static, str> {
    "ActedOn".into()
  }

  fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
      "type": "integer",
      "description": "The message this call acts on: the id of the person's message in this turn that asked for it. Omitted only in a turn whose every new message is the owner's, or that no person's message opened.",
    })
  }
}

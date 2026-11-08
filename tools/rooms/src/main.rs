//! The rooms tool's executable: one call handed by the tool host on standard input, answered on standard
//! output (docs/chat-mcp.md, 「How a call runs」).

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
  tool_call::answer_standard_io(rooms_tool::answer).await
}

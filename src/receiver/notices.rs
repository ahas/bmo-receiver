//! What the room's session is told after the owner changed one of its settings: a voiced notice
//! (docs/voiced-notices.md), queued on the receiver's instructions for the instruction pass to hand over. The facts
//! are the setting's code; the turn carries them, and what the line is for, under a marker whose rule stands in
//! the room's block.

use receiver_codes::SettingFact;
use receiver_policy::chat_round::CHAT_ROUND;
use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::{Fault, now_ms, run};

const INTENT: &str = "acknowledge the owner's command in this chat.";

/// The voiced notice's frame around the facts, for a line that is to be said.
pub fn instruction(fact: &str) -> String {
  frame(fact, INTENT, None, false)
}

/// The marker every voiced notice is handed the room's voice under, and the facts, what the line is for and what
/// follows it, as data. The rule the marker stands for is said once in the room's block.
pub fn frame(fact: &str, intent: &str, follow_up: Option<&str>, optional: bool) -> String {
  let notice = &CHAT_ROUND.notice;
  let marker = if optional { &notice.optional_marker } else { &notice.marker };
  let mut lines: Vec<String> = vec![
    marker.clone(),
    fact.trim().to_owned(),
    "END OF FACTS".to_owned(),
    String::new(),
    intent.trim().to_owned(),
  ];

  if let Some(follow_up) = follow_up.filter(|said| !said.trim().is_empty()) {
    lines.push(follow_up.trim().to_owned());
  }

  lines.join("\n")
}

/// The kind an acknowledgement's row stands under: one per setting.
pub fn kind(fact: &SettingFact) -> String {
  format!("setting:{}", fact.setting())
}

/// The acknowledgement queued for the agent's session in the room, in place of one of the same setting not yet
/// handed over.
pub async fn queue(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
  fact: &SettingFact,
) -> Result<(), Fault> {
  let kind = kind(fact);
  let now = now_ms();
  let text = instruction(&fact.to_string());
  let transaction = db.begin().await.map_err(Fault::from)?;

  run(
    &transaction,
    r#"UPDATE "instructions" SET "delivered_at" = $3, "let_go" = $4
       WHERE "chat_id" = $1 AND "kind" = $2 AND "delivered_at" IS NULL AND "handed_at" IS NULL"#,
    vec![
      chat_id.into(),
      kind.clone().into(),
      now.into(),
      crate::chat::instructions::why::REPLACED.into(),
    ],
  )
  .await?;
  run(
    &transaction,
    r#"INSERT INTO "instructions" ("agent", "chat_id", "text", "created_at", "kind", "via", "events", "rides", "ride_until")
       VALUES ($1, $2, $3, $4, $5, NULL, '', 0, NULL)"#,
    vec![
      agent.into(),
      chat_id.into(),
      text.into(),
      now.into(),
      kind.into(),
    ],
  )
  .await?;
  transaction.commit().await.map_err(Fault::from)?;

  Ok(())
}

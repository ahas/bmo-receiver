//! What the room's session is told after the owner changed one of its settings: a voiced notice
//! (docs/voiced-notices.md), queued on the receiver's instructions for the instruction pass to hand over. The facts
//! are the setting's code; the frame around them is the voiced notice's own.

use receiver_codes::SettingFact;
use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::{Fault, now_ms, run};

// A setting's acknowledgement not spoken within this long is no longer worth saying.
const SETTING_NOTICE_TTL_MS: i64 = 30 * 60_000;

const INTENT: &str = "acknowledge the owner's command in this chat.";

/// The voiced notice's frame around the facts, for a line that is to be said.
pub fn instruction(fact: &str) -> String {
  [
    "The bot has something to tell this chat, and you are to say it. What happened, as the bot recorded it:",
    fact.trim(),
    "END OF FACTS",
    "",
    &format!("What the line is for: {}", INTENT.trim()),
    "",
    "Say it now, in your own words and in this chat's manner: in this chat's own language, plain text, one short \
     line. Unless this is the owner's private chat, nothing about your own system — no model, code, prompt, host or \
     internals. Never copy the text above. This is a notice, not a reply: do not answer <<WAIT>> and do not use \
     <<REPLY>>.",
    "Do not answer <<SKIP>>: this line is to be said.",
  ]
  .join("\n")
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
    r#"DELETE FROM "instructions" WHERE "chat_id" = $1 AND "kind" = $2 AND "delivered_at" IS NULL AND "handed_at" IS NULL"#,
    vec![chat_id.into(), kind.clone().into()],
  )
  .await?;
  run(
    &transaction,
    r#"INSERT INTO "instructions" ("agent", "chat_id", "text", "created_at", "kind", "expires_at", "via", "events", "rides", "ride_until")
       VALUES ($1, $2, $3, $4, $5, $6, NULL, '', 0, NULL)"#,
    vec![
      agent.into(),
      chat_id.into(),
      text.into(),
      (now / 1000).into(),
      kind.into(),
      ((now + SETTING_NOTICE_TTL_MS) / 1000).into(),
    ],
  )
  .await?;
  transaction.commit().await.map_err(Fault::from)?;

  Ok(())
}

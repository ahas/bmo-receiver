//! A companion's turn as the receiver runs it (the agency's `docs/companions.md`, 「What a turn is given」): the round's words
//! composed into the turn — the room's lines under each speaker's name, the quiet instructions after them, the
//! reference the gates attached ahead — handed to the same session routes every agent's turns go through, and the
//! answer sent on the road the room answers on. The engine, its guard and its sessions are the agency's.

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::companion_runner::COMPANION_RUNNER;

use super::words::new_turns;
use crate::chat::ask::Carry;
use crate::chat::conversation::Carrying;
use crate::chat::conversation::{
  Asking, BoxFuture, Conversations, Failure, Follower, Held, PiecesEnded, PiecesRound, Record,
};
use crate::chat::reply::{OutgoingReply, send_reply};
use crate::chat::turnover::{judge, verdict_of};
use crate::now_ms;
use crate::receiver::{Receiver, rows};

/// The companion's turn, as the conversations hold it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Turn;

impl crate::chat::conversation::Pieces for Turn {
  /// Whether an agent's turns are a companion's own: its row takes the pieces shape of turn.
  fn of<'a>(&'a self, receiver: &'a Receiver, agent: &'a str) -> BoxFuture<'a, bool> {
    Box::pin(async move { super::Companion::of(receiver, agent).await.is_some() })
  }

  fn round<'a>(
    &'a self,
    conversations: &'a Conversations,
    agent: &'a str,
    pieces: &'a PiecesRound,
    _turn: u64,
    round: &'a str,
    _record: &'a Record,
  ) -> BoxFuture<'a, Result<PiecesEnded, Failure>> {
    Box::pin(async move {
      let receiver = &conversations.receiver();
      let chat_id = pieces.chat_id;
      let owner = receiver.deployment().owner_id;
      let identity = receiver.roads().identity_in(chat_id).await;
      let since = rows::recent_messages(receiver.db(), chat_id, CHAT_ROUND.recent_messages, "")
        .await
        .unwrap_or_default();
      let parts = new_turns(&since, &pieces.batch, &pieces.instructions, identity);
      let body = parts.join("\n\n");

      if body.trim().is_empty() {
        return Ok(PiecesEnded::NoTurn);
      }

      let asking = Asking {
        chat_id,
        session: String::new(),
        prompt: body,
        via: match pieces.via.as_str() {
          "" => None,
          road => Some(road.to_owned()),
        },
        privileged: pieces.batch.iter().all(|message| message.sender_id == Some(owner)),
        answers: pieces.ids.clone(),
        began: now_ms(),
        context: vec![],
      };
      let held = Held {
        agent: agent.to_owned(),
        chat_id,
        session: None,
        account: None,
        context_tokens: None,
        all_from_owner: asking.privileged,
        replaced: vec![],
        last_consumed_id: pieces.batch.iter().map(|message| message.message_id).max(),
      };
      let follower = Follower::new(conversations, chat_id);
      let carrying = Carry::new(receiver.clone());
      let answered = match carrying.carry(&asking, &held, &follower).await {
        Ok(answered) => answered,
        Err(why) => {
          judge(
            receiver,
            chat_id,
            held.last_consumed_id,
            &COMPANION_RUNNER.turn_stage,
            "FAILED",
            None,
            None,
          )
          .await;

          return Err(why);
        }
      };

      let sent = send_reply(
        receiver,
        &OutgoingReply {
          agent: Some(agent.to_owned()),
          chat_id,
          text: answered.text.clone(),
          reply_to: held.last_consumed_id,
          via: match pieces.via.as_str() {
            "" => None,
            road => Some(road.to_owned()),
          },
          round_started_at: None,
          round: Some(round.to_owned()),
          asked_at: None,
        },
      )
      .await;

      judge(
        receiver,
        chat_id,
        held.last_consumed_id,
        &COMPANION_RUNNER.answer_stage,
        &verdict_of(&answered.text),
        Some(&answered.text),
        None,
      )
      .await;

      Ok(PiecesEnded::Answered(sent))
    })
  }
}

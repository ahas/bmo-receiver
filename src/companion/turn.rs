//! A companion's turn as the receiver runs it (the agency's `docs/companions.md`, 「What a turn is given」; docs/wake.md,
//! 「A companion's round」): the round's words composed into the turn — the room's lines under each speaker's name,
//! the quiet instructions after them, the reference the gates attached ahead — handed to the same session routes
//! every agent's turns go through, and the answer sent on the road the room answers on. What the companion's reset
//! cut off reaches no turn, and a turn it overtook keeps nothing. The engine, its guard and its sessions are the
//! agency's.

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::companion_runner::COMPANION_RUNNER;

use super::overtaken;
use super::words::{engine_error_notice, new_turns};
use crate::chat::ask::Carry;
use crate::chat::conversation::Carrying;
use crate::chat::conversation::{
  Asking, BoxFuture, Conversations, Failure, Follower, Held, PiecesEnded, PiecesRound, Record,
};
use crate::chat::reply::{OutgoingReply, send_reply};
use crate::chat::sentinels::is_skip;
use crate::chat::turnover::{judge, verdict_of};
use crate::form::{ReplyTarget, read_reply_request};
use crate::now_ms;
use crate::receiver::{Inbound, Receiver, rows};

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
      let via = match pieces.via.as_str() {
        "" => None,
        road => Some(road.to_owned()),
      };

      // The turn begins as its round takes its words: words said and runs begun before the companion's last
      // reset are no longer its to answer, however late they reached the round.
      let began = now_ms();
      let point = overtaken::point(receiver, agent).await;
      let batch = overtaken::said_after(pieces.batch.clone(), point);
      let late = overtaken::results_before(receiver, &pieces.ids, point).await;
      let quiet: Vec<String> = pieces
        .instructions
        .iter()
        .enumerate()
        .filter(|(at, _)| pieces.ids.get(*at).is_none_or(|id| !late.contains(id)))
        .map(|(_, text)| text.clone())
        .collect();
      let recent = rows::recent_messages(receiver.db(), chat_id, CHAT_ROUND.recent_messages, "")
        .await
        .unwrap_or_default();
      // What came in since, the companion's own words left out.
      let since: Vec<Inbound> = overtaken::said_after(recent, point)
        .into_iter()
        .filter(|message| identity.is_none() || message.sender_id != identity)
        .collect();
      let body = new_turns(&since, &batch, &quiet, Some(owner)).join("\n\n");

      if (batch.is_empty() && quiet.is_empty()) || body.trim().is_empty() {
        return Ok(PiecesEnded::NoTurn);
      }

      let last = batch.iter().map(|message| message.message_id).max();
      let asking = Asking {
        chat_id,
        session: String::new(),
        prompt: body,
        via: via.clone(),
        privileged: batch.iter().all(|message| message.sender_id == Some(owner)),
        answers: batch.iter().map(|message| message.message_id).collect(),
        began,
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
        last_consumed_id: last,
      };
      let follower = Follower::new(conversations, chat_id);
      let carried = Carry::new(receiver.clone()).carry(&asking, &held, &follower).await;
      // Whatever the turn came to is kept under the hold its reset's cut-off waits on, the point read again under
      // it: a turn the reset overtook leaves the room as the reset left it, no answer and no window.
      let keeping = conversations.keeping().await;

      if matches!(carried, Err(Failure::Stopped)) || overtaken::overtaken(receiver, agent, began).await {
        println!("companion {agent} in chat {chat_id}: its reset overtook the turn, which keeps nothing");

        return Ok(PiecesEnded::NoTurn);
      }

      let answered = match carried {
        Ok(answered) => answered,
        Err(why) => {
          drop(keeping);
          judge(receiver, chat_id, last, &COMPANION_RUNNER.turn_stage, "FAILED", None, None).await;

          if let Failure::Engine { kind, .. } = &why {
            engine_error_told(receiver, agent, chat_id, via, round, kind.as_deref()).await;
          }

          eprintln!("companion {agent} in chat {chat_id}: {}", why.reason());

          return Ok(PiecesEnded::Failed {
            reason: why.reason(),
            message: last,
          });
        }
      };

      // The reply sentinel is the companion's instruction to the send and never words; an answer that is the skip
      // sentinel alone goes nowhere, and the turn ends answered.
      let (text, target) = read_reply_request(&answered.text);
      let sent = if is_skip(&text) {
        None
      } else {
        send_reply(
          receiver,
          &OutgoingReply {
            agent: Some(agent.to_owned()),
            chat_id,
            text,
            reply_to: match target {
              ReplyTarget::Plain => None,
              ReplyTarget::Last => last,
              ReplyTarget::Named(named) => Some(named),
            },
            via,
            round_started_at: Some(began),
            round: Some(round.to_owned()),
            asked_at: batch.iter().map(|message| message.sent_at).min(),
          },
        )
        .await
      };

      judge(
        receiver,
        chat_id,
        last,
        &COMPANION_RUNNER.answer_stage,
        &verdict_of(&answered.text),
        Some(&answered.text),
        None,
      )
      .await;

      Ok(PiecesEnded::Answered { sent, keeping })
    })
  }
}

// The one line a turn its engine ended with an error leaves in the room, under the companion's own name and on
// the road the room answers on; the room's language renders it.
async fn engine_error_told(
  receiver: &Receiver,
  agent: &str,
  chat_id: i64,
  via: Option<String>,
  round: &str,
  kind: Option<&str>,
) {
  let text = receiver.speech().rendered(&engine_error_notice(kind)).await;
  let told = send_reply(
    receiver,
    &OutgoingReply {
      agent: Some(agent.to_owned()),
      chat_id,
      text,
      reply_to: None,
      via,
      round_started_at: None,
      round: Some(round.to_owned()),
      asked_at: None,
    },
  )
  .await;

  if told.is_none() {
    eprintln!("companion {agent} in chat {chat_id}: the engine's error was not told to the room");
  }
}

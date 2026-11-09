//! A round's answer sent into its room (docs/receiver.md, 「The answer goes out」): the session's scaffolding and
//! the send's own markers taken off, the dashes replaced, and sent on the road the turn
//! answers on — the one its words came in on where that road reaches the room — in the one send that road divides
//! into its parts, the first pinned to the message it answers, folded only where the answer's own mark asks it
//! (docs/wake.md, 「Folds」). What went out names the round that wrote it, is attached to the run the round's turn
//! opened at the agency, and the send's own verdict is written beside the round's.

use bmo_chat_client::{ChatSend, ChatSendRequest};

use super::conversation::{BoxFuture, Reply, ReplyRoad, Sent};
use super::sentinels::asks_fold;
use super::turnover::judge;
use crate::form::{outgoing, strip_prompt_scaffolding, without_control_markers};
use crate::now_ms;
use crate::receiver::speech::KeptLine;
use crate::receiver::{Receiver, rows};
use crate::roads::providers::Carried;
use crate::roads::{Road, added};

/// An answer bound for a room: the agent whose answer it is, its words, the message it answers, the road its words
/// came in on, when the round that wrote it began and its name, and when the room said the earliest thing it
/// answers. An answer naming no agent is the default answerer's.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OutgoingReply {
  pub agent: Option<String>,
  pub chat_id: i64,
  pub text: String,
  #[serde(rename = "replyToMessageId")]
  pub reply_to: Option<i64>,
  pub via: Option<String>,
  /// When the round began, in epoch milliseconds, which the round's judgements are attached from; none attaches
  /// only what the send itself is judged by.
  pub round_started_at: Option<i64>,
  pub round: Option<String>,
  pub asked_at: Option<i64>,
}

// The round's judgements that were written before the message existed, filed by the gate, whose they are, under
// the message that answers (docs/boundary-api.md, 「The gate」).
async fn attach_judgements(receiver: &Receiver, chat_id: i64, message_id: i64, since: i64) {
  if let Err(why) = receiver.units().gate.attach(chat_id, message_id, since).await {
    eprintln!("chat {chat_id}: the round's judgements could not be attached: {why}");
  }
}

async fn mark_round(receiver: &Receiver, chat_id: i64, message_id: i64, round: Option<&str>) {
  let Some(round) = round else {
    return;
  };

  if let Err(why) = rows::mark_message_round(receiver.db(), chat_id, message_id, round).await {
    eprintln!("chat {chat_id}: message {message_id} could not be marked: {}", why.detail);
  }
}

// The answer, as the room keeps it, attached to the newest run the round's turns opened in the agency's ledger
// (docs/boundary-api.md, 「The agency」), so the message reads back the run that wrote it.
async fn attach_run(receiver: &Receiver, chat_id: i64, message_id: i64, round: Option<&str>) {
  let Some(round) = round else {
    return;
  };
  let agency = &receiver.units().agency;
  let run = match agency.round_runs(round).await {
    // The round's last run is its newest, the highest id, whatever order the agency answers in.
    Ok(runs) => runs.iter().max().copied(),
    Err(why) => {
      eprintln!("chat {chat_id}: the runs of {round} could not be read: {why}");

      return;
    }
  };
  // A round whose turn never reached the agency opened no run.
  let Some(run) = run else {
    return;
  };

  if let Err(why) = agency.attach_message(run, message_id).await {
    eprintln!("chat {chat_id}: message {message_id} could not be attached to run {run}: {why}");
  }
}

async fn answer_wait(receiver: &Receiver, chat_id: i64, message_id: i64, asked_at: Option<i64>) {
  if let Some(asked_at) = asked_at {
    let sent_at = now_ms();

    if let Err(why) = rows::record_answer_wait(receiver.db(), chat_id, message_id, asked_at, sent_at).await
    {
      eprintln!("chat {chat_id}: the answer's wait could not be written: {}", why.detail);
    }
  }
}

/// What an agent speaking under its own name is called in a room: its label, or its name where it has none.
pub async fn speaker_label(receiver: &Receiver, agent: &str) -> String {
  let listed = receiver.agents().await.ok();

  listed
    .as_ref()
    .and_then(|listed| listed.agents.iter().find(|one| one.name == agent))
    .and_then(|found| found.label.clone())
    .unwrap_or_else(|| agent.to_owned())
}

/// The answer sent: the message it went out as and its words as the room received them, or none where nothing
/// reached the room.
pub async fn send_reply(receiver: &Receiver, reply: &OutgoingReply) -> Option<Sent> {
  let chat_id = reply.chat_id;
  let written = strip_prompt_scaffolding(&reply.text);
  let fold = asks_fold(&written);
  let stripped = without_control_markers(&written);

  if stripped.is_empty() {
    eprintln!("refused to send prompt scaffolding to chat {chat_id}");

    return None;
  }

  let started = now_ms();
  let text = outgoing(&stripped);
  let road = receiver.roads().answering(chat_id, reply.via.as_deref()).await;

  match road {
    None => {
      eprintln!("chat {chat_id}: no road to answer on");

      None
    }
    Some(road) => carry(receiver, road, reply, &text, fold, started).await,
  }
}

// One answer carried on its road, under the agent that says it where that is not the road's own voice, folded
// where the agent asked it.
async fn carry(
  receiver: &Receiver,
  road: Road,
  reply: &OutgoingReply,
  text: &str,
  fold: bool,
  started: i64,
) -> Option<Sent> {
  let chat_id = reply.chat_id;
  let send = ChatSend::Text {
    text: text.to_owned(),
    silent: false,
    fold,
  };
  let mut request = ChatSendRequest::new(chat_id, send.clone());
  request.reply_to_message_id = reply.reply_to;
  let agent = reply.agent.clone().unwrap_or_else(|| receiver.default_answerer());
  // The answer as the room keeps it, which goes on to the room's other providers as the same send, its fold
  // with it.
  let line = KeptLine {
    chat_id,
    text,
    reply_to: reply.reply_to,
    sent: None,
    agent: Some(&agent),
    voice: None,
  };
  // The number the answer went out as, and the number the room keeps it under.
  let numbers = match road {
    Road::Record => receiver
      .speech()
      .kept_carrying(&line, Carried::Send(send.clone()))
      .await
      .ok()
      .map(|kept| (kept, kept)),
    Road::Service(key) => match receiver.speech().act(&key, &request).await.ok().flatten() {
      // A service's road sends through that service; the room's own store keeps a copy beside it, which
      // the console and every other reader of the room's conversation read, and which goes on to the room's
      // providers but that road. The copy is the answer as the room holds it: the round's judgements, its
      // mark and its wait are filed under it, where it was kept. The copy names the agent that answered and
      // the voice the send went out in, the room's avatar voice where it has one.
      Some(id) => {
        let voice = added::voice_in(receiver.db(), chat_id).await;
        let line = KeptLine {
          sent: Some((&key, Some(id))),
          voice,
          ..line
        };
        Some((
          id,
          receiver.speech().kept_carrying(&line, Carried::Send(send.clone())).await.unwrap_or(id),
        ))
      }
      None => None,
    },
  };

  let Some((message_id, kept)) = numbers else {
    judge(receiver, chat_id, None, "send", "FAILED", None, Some(now_ms() - started)).await;

    return None;
  };

  mark_round(receiver, chat_id, kept, reply.round.as_deref()).await;
  attach_run(receiver, chat_id, kept, reply.round.as_deref()).await;
  answer_wait(receiver, chat_id, kept, reply.asked_at).await;

  judge(receiver, chat_id, Some(kept), "send", "SENT", None, Some(now_ms() - started)).await;
  attach_judgements(receiver, chat_id, kept, reply.round_started_at.unwrap_or(started)).await;

  Some(Sent {
    message_id,
    text: text.to_owned(),
  })
}

/// The sending, as the conversations hold it.
#[derive(Clone)]
pub struct Sender(Receiver);

impl Sender {
  pub fn new(receiver: Receiver) -> Self {
    Sender(receiver)
  }
}

impl crate::chat::conversation::Sending for Sender {
  fn send<'a>(
    &'a self,
    _receiver: &'a Receiver,
    agent: &'a str,
    road: &'a ReplyRoad,
    reply: &'a Reply,
  ) -> BoxFuture<'a, Option<Sent>> {
    Box::pin(async move {
      send_reply(
        &self.0,
        &OutgoingReply {
          agent: Some(agent.to_owned()),
          chat_id: road.chat_id,
          text: reply.text.clone(),
          reply_to: road.reply_to,
          via: match road.via.as_str() {
            "" => None,
            via => Some(via.to_owned()),
          },
          round_started_at: Some(road.began),
          round: Some(road.round.to_owned()),
          asked_at: road.asked_at,
        },
      )
      .await
    })
  }
}

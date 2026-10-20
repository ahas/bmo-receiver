//! A round's answer sent into its room (docs/receiver.md, 「The answer goes out」): the session's scaffolding and
//! the send's own markers taken off, the words cleaned under the agent's own switch, and sent on the road the turn
//! answers on — the one its words came in on where that road reaches the room — in the one send that road folds
//! into its parts, the first pinned to the message it answers. What went out names the round that wrote it, and
//! the send's own verdict is written beside the round's.

use bmo_chat_client::{ChatSend, ChatSendRequest};

use super::conversation::{BoxFuture, Reply, ReplyRoad, Sent};
use super::turnover::judge;
use super::window::gate_on;
use crate::form::{message_label, outgoing, strip_prompt_scaffolding, without_control_markers};
use crate::now_ms;
use crate::receiver::{Receiver, rows};
use crate::roads::Road;

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

async fn answer_wait(receiver: &Receiver, chat_id: i64, message_id: i64, asked_at: Option<i64>) {
  if let Some(asked_at) = asked_at {
    let sent_at = now_ms() / 1000;

    if let Err(why) = rows::record_answer_wait(receiver.db(), chat_id, message_id, asked_at, sent_at).await
    {
      eprintln!("chat {chat_id}: the answer's wait could not be written: {}", why.detail);
    }
  }
}

/// What an agent speaking under its own name is called in a room: its label, or its name where it has none.
pub async fn speaker_label(receiver: &Receiver, agent: &str) -> String {
  let listed = receiver.units().agency.agents().await.ok();

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
  let stripped = without_control_markers(&strip_prompt_scaffolding(&reply.text));

  if stripped.is_empty() {
    eprintln!("refused to send prompt scaffolding to chat {chat_id}");

    return None;
  }

  let started = now_ms();
  let default = receiver.deployment().agent.clone();
  let agent = reply.agent.clone().unwrap_or_else(|| default.to_owned());
  let cleaned = gate_on(receiver, &agent, "sanitizer").await;
  let text = match outgoing(&*receiver.units().gate, chat_id, &stripped, &message_label(), cleaned).await {
    Ok(text) => text,
    Err(why) => {
      eprintln!("chat {chat_id}: the answer could not be cleaned: {why}");

      return None;
    }
  };
  let road = receiver.roads().answering(chat_id, reply.via.as_deref()).await;

  let sent = match road {
    None => {
      eprintln!("chat {chat_id}: no road to answer on");

      None
    }
    Some(road) => carry(receiver, road, reply, &text, started).await,
  };

  if let Some(sent) = &sent {
    attach_judgements(
      receiver,
      chat_id,
      sent.message_id,
      reply.round_started_at.unwrap_or_else(|| now_ms() / 1000),
    )
    .await;
  }

  sent
}

// One answer carried on its road, under the agent that says it where that is not the road's own voice.
async fn carry(
  receiver: &Receiver,
  road: Road,
  reply: &OutgoingReply,
  text: &str,
  started: i64,
) -> Option<Sent> {
  let chat_id = reply.chat_id;
  let send = ChatSend::Text {
    text: text.to_owned(),
    silent: false,
    fold: true,
  };
  let mut request = ChatSendRequest::new(chat_id, send);
  request.reply_to_message_id = reply.reply_to;
  let message_id = match road {
    Road::Record => receiver.speech().kept(chat_id, text).await.ok(),
    Road::Service(key) => receiver.speech().act(&key, &request).await.ok().flatten(),
  };

  let Some(message_id) = message_id else {
    judge(receiver, chat_id, None, "send", "FAILED", None, Some(now_ms() - started)).await;

    return None;
  };

  mark_round(receiver, chat_id, message_id, reply.round.as_deref()).await;
  answer_wait(receiver, chat_id, message_id, reply.asked_at).await;

  judge(receiver, chat_id, None, "send", "SENT", None, Some(now_ms() - started)).await;

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
          round_started_at: None,
          round: Some(road.round.to_owned()),
          asked_at: road.asked_at,
        },
      )
      .await
    })
  }
}

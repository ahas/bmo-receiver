//! A round's answer sent into its room (docs/receiver.md, 「The answer goes out」): the session's scaffolding and
//! the send's own markers taken off, the dashes replaced, and sent on the road the turn
//! answers on — the one its words came in on where that road reaches the room — in the one send that road divides
//! into its parts, the first pinned to the message it answers, folded only where the answer's own mark asks it
//! (docs/wake.md, 「Folds」). An answer its round asked said aloud goes out as a voice note in the agent's own voice,
//! its words beside it, where the voice engine spoke it, and written where it did not (docs/voiced-notices.md, 「An
//! answer said aloud」). What went out names the round that wrote it, is attached to the run the round's turn
//! opened at the agency, and the send's own verdict is written beside the round's.

use bmo_chat_client::{ChatSend, ChatSendRequest, FileBody};

use super::conversation::{BoxFuture, Reply, ReplyRoad, Sent, Unsent};
use super::sentinels::asks_fold;
use super::spoken::{self, Spoken, Voiced};
use super::turnover::judge;
use crate::form::{outgoing, strip_prompt_scaffolding, without_control_markers};
use crate::now_ms;
use crate::receiver::speech::KeptLine;
use crate::receiver::{Receiver, rows};
use crate::roads::providers::Carried;
use crate::roads::{Road, added};
use crate::verbs::sends::{keep_sent_files, kept_line};

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
  /// How the answer goes out said aloud, where its round asked it with speak.
  #[serde(skip)]
  pub spoken: Option<Spoken>,
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

/// The answer sent: the message it went out as and its words as the room received them, or why it did not reach
/// the room, which the journal is told too.
pub async fn send_reply(receiver: &Receiver, reply: &OutgoingReply) -> Result<Sent, Unsent> {
  let chat_id = reply.chat_id;
  let written = strip_prompt_scaffolding(&reply.text);
  let fold = asks_fold(&written);
  let stripped = without_control_markers(&written);

  if stripped.is_empty() {
    eprintln!("refused to send prompt scaffolding to chat {chat_id}");

    return Err(Unsent::nothing_went("the answer held nothing but prompt scaffolding"));
  }

  let started = now_ms();
  let text = outgoing(&stripped);
  let road = receiver.roads().answering(chat_id, reply.via.as_deref()).await;

  match road {
    None => {
      eprintln!("chat {chat_id}: no road to answer on");

      Err(Unsent::nothing_went("no road answers the room"))
    }
    Some(road) => {
      let voiced = voiced(receiver, &road, reply, &text).await;

      carry(receiver, road, reply, &text, fold, started, voiced).await
    }
  }
}

// The answer's speech, where its round asked it said aloud and the road performs a voice note; none where it did
// not, and none where the speech did not stand, which the journal is told, so the answer goes out written.
async fn voiced(receiver: &Receiver, road: &Road, reply: &OutgoingReply, text: &str) -> Option<Voiced> {
  let spoken = reply.spoken.as_ref()?;
  let chat_id = reply.chat_id;

  if !receiver.roads().facts(road).await.can("voice") {
    eprintln!("chat {chat_id}: the road answering performs no voice note, so the answer goes out written");

    return None;
  }

  let agent = reply.agent.clone().unwrap_or_else(|| receiver.default_answerer());
  let key = reply.round.as_deref().map(|round| format!("receiver:speech:{round}"));

  match spoken::voiced(receiver, &agent, chat_id, text, spoken, key).await {
    Ok(voiced) => Some(voiced),
    Err(why) => {
      eprintln!("chat {chat_id}: the answer was not said aloud, so it goes out written: {why}");

      None
    }
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
  voiced: Option<Voiced>,
) -> Result<Sent, Unsent> {
  let chat_id = reply.chat_id;
  // A voice note says the answer and carries its words, which a road folds under it.
  let send = match &voiced {
    Some(voiced) => ChatSend::Voice {
      audio: FileBody::Inline(voiced.sound.clone()),
      said: text.to_owned(),
      caption: None,
      transcript: None,
    },
    None => ChatSend::Text {
      text: text.to_owned(),
      silent: false,
      fold,
    },
  };
  let mut request = ChatSendRequest::new(chat_id, send.clone());
  request.reply_to_message_id = reply.reply_to;
  let agent = reply.agent.clone().unwrap_or_else(|| receiver.default_answerer());
  let kept_text = kept_line(&send);
  let road_key = road.key().to_owned();
  // The answer as the room keeps it, which goes on to the room's other providers as the same send, its fold
  // with it.
  let line = KeptLine {
    chat_id,
    text: &kept_text,
    reply_to: reply.reply_to,
    sent: None,
    agent: Some(&agent),
    voice: None,
    line: None,
  };
  // The number the answer went out as, and the number the room keeps it under.
  let numbers = match road {
    Road::Record => receiver
      .speech()
      .kept_carrying(&line, Carried::Send(send.clone()))
      .await
      .map(|kept| (kept, kept))
      .map_err(|why| Unsent::nothing_went(why.to_string())),
    Road::Service(key) => match receiver.speech().act(&key, &request).await {
      // A service's road sends through that service; the room's own store keeps a copy beside it, which
      // the console and every other reader of the room's conversation read, and which goes on to the room's
      // providers but that road. The copy is the answer as the room holds it: the round's judgements, its
      // mark and its wait are filed under it, where it was kept. The copy names the agent that answered and
      // the voice the send went out in, the one the road answers the room by where it names one. A road that
      // carried the line and gave it no number leaves it in the room all the same, kept under the number the
      // record gives it.
      Ok(id) => {
        let voice = added::voice_in(receiver.db(), receiver.roads(), &key, chat_id).await;
        let line = KeptLine {
          sent: Some((&key, id)),
          voice,
          ..line
        };
        let kept = receiver.speech().kept_carrying(&line, Carried::Send(send.clone())).await;

        match (id, kept) {
          (_, Ok(kept)) => Ok((id.unwrap_or(kept), kept)),
          (Some(id), Err(_)) => Ok((id, id)),
          (None, Err(why)) => Err(Unsent {
            reason: format!(
              "the {key} road carried the line with no number, and the room did not keep it: {why}"
            ),
            may_have_gone: true,
          }),
        }
      }
      Err(why) => Err(Unsent {
        reason: why.to_string(),
        may_have_gone: why.may_have_reached(),
      }),
    },
  };

  // A send that put nothing into the room says why, in the journal and in the send's verdict.
  let (message_id, kept) = match numbers {
    Ok(numbers) => numbers,
    Err(unsent) => {
      eprintln!("chat {chat_id}: the answer was not sent: {}", unsent.reason);
      judge(
        receiver,
        chat_id,
        None,
        "send",
        "FAILED",
        Some(&unsent.reason),
        Some(now_ms() - started),
      )
      .await;

      return Err(unsent);
    }
  };

  // A voice note's sound is kept with the message the room keeps, and its speech is told the line it went out as.
  if let Some(voiced) = &voiced {
    if let Err(why) = keep_sent_files(receiver, &road_key, &request, kept, &[]).await {
      eprintln!("chat {chat_id}: the answer's voice note was not kept: {}", why.detail);
    }

    spoken::name_line(receiver, chat_id, voiced.briefing, message_id).await;
  }

  mark_round(receiver, chat_id, kept, reply.round.as_deref()).await;
  attach_run(receiver, chat_id, kept, reply.round.as_deref()).await;
  answer_wait(receiver, chat_id, kept, reply.asked_at).await;

  judge(receiver, chat_id, Some(kept), "send", "SENT", None, Some(now_ms() - started)).await;
  attach_judgements(receiver, chat_id, kept, reply.round_started_at.unwrap_or(started)).await;

  Ok(Sent {
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
  ) -> BoxFuture<'a, Result<Sent, Unsent>> {
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
          spoken: road.spoken.clone(),
        },
      )
      .await
    })
  }
}

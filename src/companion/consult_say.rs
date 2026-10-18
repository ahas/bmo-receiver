//! One line in the consult room under an agent's own name (docs/consult.md, 「The consult tool」): on the room's
//! own road — the agent's words through the outgoing sanitiser under its own switch — filed under the agent's
//! number, opening the agent's own window where its row keeps one, and handed to the receiver like any word filed
//! in the room, so the agent it calls is woken by the receiver's one rule.

use bmo_chat_client::{ChatSend, ChatSendRequest};
use receiver_codes::ConsultRefusal;
use receiver_policy::consult::CONSULT;
use receiver_policy::fill_marks;

use super::consult::{label_of, window_line, window_states};
use crate::chat::window::gate_on;
use crate::form::outgoing;
use crate::now_ms;
use crate::receiver::wake::agents::{open_window, room_designated};
use crate::receiver::{NotReceived, Receiver, Room, rows};
use crate::roads::{Road, record};
use crate::store::settings;

/// What a line said in the consult room came to: the room, the message it went out as where its road gave one,
/// and the lines the caller is answered with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said {
  pub chat_id: i64,
  pub message_id: Option<i64>,
  pub lines: Vec<String>,
}

// The room as a person reads it named: by its title where it has one.
async fn room_name(receiver: &Receiver, chat_id: i64) -> Result<String, NotReceived> {
  let chat = chat_id.to_string();

  Ok(match rows::chat_title(receiver.db(), chat_id).await? {
    Some(title) => fill_marks(&CONSULT.work.room_titled, &[("title", &title), ("chat", &chat)]),
    None => fill_marks(&CONSULT.work.room_bare, &[("chat", &chat)]),
  })
}

/// The line said, or the one refusal saying why it was not.
pub async fn say(
  receiver: Receiver,
  agent: &str,
  text: &str,
  reply_to: Option<i64>,
) -> Result<Result<Said, ConsultRefusal>, NotReceived> {
  let db = receiver.db();
  let text = text.trim();

  if text.is_empty() {
    return Ok(Err(ConsultRefusal::Empty));
  }
  let set = settings::get(db, &CONSULT.room_setting)
    .await?
    .and_then(|stored| stored.trim().parse::<i64>().ok());
  let Some(chat_id) = set else { return Ok(Err(ConsultRefusal::NoRoom)) };
  let room = receiver.roads().room_facts(chat_id).await;
  let (answerers, _) = room_designated(&receiver, chat_id, room).await?;
  let name = label_of(&receiver, agent).await;
  let Some(designated) = answerers.iter().find(|one| one.name == agent) else {
    let room = room_name(&receiver, chat_id).await?;

    return Ok(Err(ConsultRefusal::NotAnswerer {
      agent: agent.to_owned(),
      room: room.clone(),
    }));
  };
  let chat = designated.answers_unnamed();
  let words = if chat {
    text.to_owned()
  } else {
    let label = fill_marks(&CONSULT.sanitised, &[("name", &name)]);
    let cleaned = gate_on(&receiver, agent, "sanitizer").await;

    outgoing(&*receiver.units().gate, chat_id, text, &label, cleaned)
      .await
      .map_err(NotReceived::Unit)?
  };
  let road = receiver.roads().answering(chat_id, None).await;
  let via = match &road {
    Some(Road::Service(key)) => key.clone(),
    _ => Road::Record.key().to_string(),
  };
  let message_id = match road {
    Some(Road::Record) => {
      let numbered = record::agent_number(db, &via, agent).await.map_err(NotReceived::Store)?;
      let kept = record::Kept {
        chat_id,
        text: words.as_ref(),
        reply_to,
        via: Road::Record.key(),
        sender_id: numbered,
        name: name.as_ref(),
      };

      Some(record::keep(db, receiver.news(), kept).await.map_err(NotReceived::Store)?)
    }
    Some(Road::Service(key)) => {
      let send = ChatSend::Text {
        text: words.clone(),
        reply_to,
        silent: false,
        fold: false,
      };

      receiver
        .speech()
        .act(&key, &ChatSendRequest::new(chat_id, send))
        .await
        .map_err(NotReceived::Said)?
    }
    None => return Ok(Err(ConsultRefusal::NoRoom)),
  };

  if let Some(seconds) = designated.window_seconds {
    open_window(db, chat_id, agent, now_ms() / 1000 + seconds).await?;
    receiver.news().window(chat_id);
  }

  if let Some(message_id) = message_id {
    hand_filed(receiver.clone(), chat_id, message_id, via, room).await;
  }

  let states = window_states(&receiver, chat_id).await;
  let now = now_ms() / 1000;
  let number =
    message_id.map_or(String::new(), |id| fill_marks(&CONSULT.number, &[("id", &id.to_string())]));
  let room = room_name(&receiver, chat_id).await?;
  let mut lines = vec![
    fill_marks(&CONSULT.said, &[("room", &room), ("name", &name), ("number", &number)]),
    CONSULT.windows.heading.clone(),
  ];

  lines.extend(states.iter().map(|state| window_line(state, agent, now)));

  Ok(Ok(Said {
    chat_id,
    message_id,
    lines,
  }))
}

// The line handed to the receiver as filed words, apart from the answer: the agent the line calls is woken by the
// receiver's rule.
async fn hand_filed(receiver: Receiver, chat_id: i64, message_id: i64, via: String, room: Room) {
  match receiver.filed(chat_id, message_id, &via, room).await {
    Ok(()) => (),
    Err(why) => {
      eprintln!("consult room {chat_id}: filed words {message_id} were not handed to the receiver: {why}")
    }
  }
}

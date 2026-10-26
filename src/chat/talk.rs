//! A talk the owner has with the chat agent from the console about one thing he has open there — an item in the
//! item talk panel, a document in the document viewer (docs/boundary-api.md, 「For bmo-job」, `POST /talks`). The
//! chat agent itself answers: the one session every room shares takes the turn as a round of the owner's private
//! chat, in the same line as every room's rounds, and its answer is handed back to the console alone. What a turn
//! is about is its subject's: the head it opens on and what the owner's words are called as they are cleaned. The
//! session keeps its own memory of the talk, but it is folded and replaced over time, so a turn restates the last
//! few turns.

use receiver_policy::fill_marks;
use receiver_policy::talk::TALK;

use super::ask::Carry;
use super::conversation::{Asking, BoxFuture, Conversations, Failure, Follower, Talking};
use super::window::{Material, clean, composed, select};

use crate::chat::consult::label_of;
use crate::now_ms;
use crate::receiver::{Inbound, Receiver};

/// One earlier turn as a turn restates it: whether the owner said it, and its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaidTurn {
  pub owner: bool,
  pub text: String,
}

/// A part of a turn's head and whether it is the owner's own text, which is cleaned before it goes in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadPart {
  /// A line of the head as the subject writes it.
  Line(String),
  /// Text the owner did not write the head around, cleaned and placed in the line by `{text}`.
  Cleaned { line: String, text: String },
}

/// One turn asked: what the owner's words are called as they are cleaned, the head the subject opens on, the
/// turns before this one and what he says now.
#[derive(Clone, Debug)]
pub struct TalkRequest {
  pub words: String,
  pub head: Vec<HeadPart>,
  pub turns: Vec<SaidTurn>,
  pub said: String,
}

/// Why a turn came back with nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TalkFailure {
  /// The chat agent rests, or the owner's chat has fallen silent.
  Resting,
  Limit,
  Outage,
  Other(String),
}

/// Where one talk stands in the process: whether a turn is being answered, and why the last went unanswered.
/// Neither is kept anywhere else, since a turn is a round of the session rather than a row: a restart ends it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TalkState {
  pub answering: bool,
  pub failure: Option<String>,
}

/// Where every talk stands, and what takes their turns.
#[derive(Default)]
pub struct Standing {
  states: std::sync::Mutex<std::collections::HashMap<String, TalkState>>,
  taker: std::sync::OnceLock<std::sync::Arc<dyn Talking>>,
}

impl Standing {
  fn states(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, TalkState>> {
    self.states.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  /// What takes the turns from now on; the first given stands.
  pub fn hold(&self, taker: std::sync::Arc<dyn Talking>) {
    let _ = self.taker.set(taker);
  }

  pub fn taker(&self) -> Option<std::sync::Arc<dyn Talking>> {
    self.taker.get().cloned()
  }

  /// Where one talk stands now.
  pub fn state(&self, key: &str) -> TalkState {
    self.states().get(key).cloned().unwrap_or_default()
  }

  /// The talk taken into a turn, where it is not on one already: whether it was.
  pub fn begin(&self, key: &str) -> bool {
    let mut states = self.states();
    let state = states.entry(key.to_owned()).or_default();

    if state.answering {
      return false;
    }

    *state = TalkState {
      answering: true,
      failure: None,
    };

    true
  }

  /// The talk's turn over, with why it went unanswered where it did.
  pub fn end(&self, key: &str, failure: Option<String>) {
    self.states().insert(
      key.to_owned(),
      TalkState {
        answering: false,
        failure: failure.clone(),
      },
    );
  }
}

/// Words clipped to so many characters, an ellipsis marking the cut.
pub fn clip(text: &str, limit: usize) -> String {
  if text.chars().count() <= limit {
    text.to_owned()
  } else {
    format!("{}…", text.chars().take(limit).collect::<String>())
  }
}

/// The turn as the session reads it: the head, the last few turns restated, and what he says now. The owner's
/// name and the chat agent's label stand in the composition's lines in place of the policy's marks.
pub fn prompt(head: &[String], turns: &[SaidTurn], said: &str, owner: &str, agent: &str) -> String {
  let mut lines: Vec<String> = head.to_vec();
  let earlier = &turns[turns.len().saturating_sub(TALK.turns_given)..];
  let names = [("owner", owner), ("agent", agent)];

  if !earlier.is_empty() {
    lines.push(String::new());
    lines.push(fill_marks(
      &TALK.earlier,
      &[("shown", &earlier.len().to_string()), ("all", &turns.len().to_string())],
    ));

    for turn in earlier {
      let speaker = if turn.owner { &TALK.owner } else { &TALK.agent };

      lines.push(format!("{}: {}", fill_marks(speaker, &names), clip(&turn.text, TALK.turn_clip)));
    }

    lines.push(TALK.earlier_end.clone());
  }

  lines.push(String::new());
  lines.push(fill_marks(&TALK.says_now, &names));
  lines.push(said.to_owned());
  lines.join("\n")
}

/// An item as its talk's head names it.
pub struct ItemTopic {
  pub id: i64,
  pub title: String,
  pub done: bool,
  pub merged_into: Option<i64>,
  pub held: bool,
  /// Why it is held, where it is: `stop`, `pause`, `failure` or any other hold.
  pub hold: Option<String>,
  /// Whether a run of it still ends while it is held.
  pub stopping: bool,
  pub approved: bool,
  pub status: String,
}

/// What the item's state is called in its talk's head.
pub fn item_state(item: &ItemTopic) -> String {
  let states = &TALK.item.states;

  if item.done {
    return states.done.clone();
  }

  if let Some(into) = item.merged_into {
    return fill_marks(&states.merged, &[("into", &into.to_string())]);
  }

  if item.held {
    return match (item.hold.as_deref(), item.stopping) {
      (Some("stop"), true) => states.stop_pending.clone(),
      (Some("stop"), false) => states.stop.clone(),
      (Some("pause"), true) => states.pause_pending.clone(),
      (Some("pause"), false) => states.pause.clone(),
      (_, true) => states.held_pending.clone(),
      (Some("failure"), false) => states.held_failure.clone(),
      _ => states.held.clone(),
    };
  }

  if item.approved { item.status.clone() } else { states.waiting.clone() }
}

/// The talk about one item: its record is not copied in, since the session reads it with the TODO verb as it
/// stands now.
pub fn item_request(item: &ItemTopic, turns: Vec<SaidTurn>, said: String) -> TalkRequest {
  let id = item.id.to_string();
  let state = item_state(item);
  let marks = [("id", id.as_str()), ("title", item.title.as_str()), ("state", state.as_str())];

  TalkRequest {
    words: TALK.item.words.clone(),
    head: TALK.item.head.iter().map(|line| HeadPart::Line(fill_marks(line, &marks))).collect(),
    turns,
    said,
  }
}

/// A document as its talk's head names it: its name, its root's label and writer, its file here and the
/// version he was reading, and the passage he selected under its heading.
pub struct DocumentTopic {
  pub document: String,
  pub label: String,
  pub writer: String,
  pub file: String,
  pub version: String,
  pub heading: Option<String>,
  pub passage: Option<String>,
  pub translated: bool,
}

/// The talk about one document: the head, then the passage he selected, cleaned, where he selected one.
pub fn document_request(topic: &DocumentTopic, turns: Vec<SaidTurn>, said: String) -> TalkRequest {
  let lines = &TALK.document;
  let version: String = topic.version.chars().take(12).collect();
  let marks = [
    ("document", topic.document.as_str()),
    ("label", topic.label.as_str()),
    ("writer", topic.writer.as_str()),
    ("file", topic.file.as_str()),
    ("version", version.as_str()),
  ];
  let mut head: Vec<HeadPart> =
    lines.head.iter().map(|line| HeadPart::Line(fill_marks(line, &marks))).collect();

  if let Some(passage) = &topic.passage {
    let translated = if topic.translated { lines.translated.as_str() } else { "" };

    head.push(HeadPart::Line(String::new()));

    // The heading goes in cleaned, inside the line that names it.
    match &topic.heading {
      Some(heading) => head.push(HeadPart::Cleaned {
        line: fill_marks(
          &lines.passage,
          &[
            ("under", &fill_marks(&lines.under, &[("heading", "{text}")])),
            ("translated", translated),
          ],
        ),
        text: heading.clone(),
      }),
      None => {
        head.push(HeadPart::Line(fill_marks(&lines.passage, &[("under", ""), ("translated", translated)])))
      }
    }

    head.push(HeadPart::Cleaned {
      line: "{text}".to_owned(),
      text: clip(passage, lines.passage_clip),
    });
    head.push(HeadPart::Line(lines.passage_end.clone()));
  }

  TalkRequest {
    words: lines.words.clone(),
    head,
    turns,
    said,
  }
}

/// The talk about anything a caller names: the kind of thing it is, the head paragraph the caller writes and the
/// fields it hands, with no kind enumerated here. A caller's own subject needs no code in the receiver.
pub fn general_request(
  kind: &str,
  heading: Option<&str>,
  fields: &serde_json::Map<String, serde_json::Value>,
  turns: Vec<SaidTurn>,
  said: String,
) -> TalkRequest {
  let mut head = vec![HeadPart::Line(format!("This talk is about a {kind}."))];

  if let Some(heading) = heading.filter(|line| !line.trim().is_empty()) {
    head.push(HeadPart::Cleaned {
      line: "{text}".to_owned(),
      text: heading.to_owned(),
    });
  }

  for (name, value) in fields {
    let written = value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string());

    head.push(HeadPart::Line(format!("{name}: {written}")));
  }

  TalkRequest {
    words: format!("the {kind} the owner is talking about"),
    head,
    turns,
    said,
  }
}

/// A turn's failure as the talk reads it: a rest, a limit, an outage, or the reason beside it.
fn failure_of(failure: Failure) -> TalkFailure {
  match failure {
    Failure::Stopped => TalkFailure::Resting,
    Failure::Limit => TalkFailure::Limit,
    Failure::Outage => TalkFailure::Outage,
    Failure::Transport(why) | Failure::Failed(why) | Failure::Engine { reason: why, .. } => {
      TalkFailure::Other(why.clone())
    }
  }
}

/// A talk's turn as the chat agent's conversations take it: the turn cleaned, restated and asked of the session
/// the owner's chat resumes, whose answer is handed back rather than sent. The conversations hold their turn
/// around it.
pub async fn answer(
  conversations: &Conversations,
  request: &TalkRequest,
) -> Result<Option<String>, TalkFailure> {
  let owner = conversations.owner();
  let receiver = conversations.receiver();

  if conversations.silenced(owner).await {
    return Err(TalkFailure::Resting);
  }

  let clean =
    |text: String| clean(&receiver, conversations.agent(), text, request.words.clone(), owner, None);
  let said = match clean(request.said.clone()).await {
    Ok(said) => said,
    Err(why) => return Err(TalkFailure::Other(why.detail)),
  };
  let mut head = Vec::new();

  for part in &request.head {
    head.push(match part {
      HeadPart::Line(line) => line.clone(),
      HeadPart::Cleaned { line, text } => match clean(text.clone()).await {
        Ok(cleaned) => line.replace("{text}", &cleaned),
        Err(why) => return Err(TalkFailure::Other(why.detail)),
      },
    });
  }

  let mut turns = Vec::new();

  for turn in &request.turns {
    match clean(turn.text.clone()).await {
      Ok(text) => turns.push(SaidTurn {
        owner: turn.owner,
        text: text.clone(),
      }),
      Err(why) => return Err(TalkFailure::Other(why.detail)),
    }
  }

  let names = (receiver.owner_name(owner).await, agent_label(&receiver, conversations.agent()).await);
  let written = prompt(&head, &turns, &said, &names.0, &names.1);
  let room = receiver.roads().room_facts(owner).await;
  let messages: Vec<Inbound> = vec![];
  let recent: Vec<String> = vec![];
  let instructions: Vec<String> = vec![];
  let events: Vec<receiver_ports::gate::TurnEvent> = vec![];
  let material = Material {
    agent: conversations.agent(),
    chat_id: owner,
    room,
    messages: &messages,
    instructions: &instructions,
    said: std::slice::from_ref(&said),
    events: &events,
    recent: &recent,
    holds: None,
    round: None,
    session: None,
  };
  let selection = match select(&receiver, &material).await {
    Ok(selection) => selection,
    Err(why) => return Err(TalkFailure::Other(why.detail)),
  };
  let held = conversations.held_session(owner).await;
  let asking = Asking {
    chat_id: owner,
    session: held.session.clone().unwrap_or_default(),
    prompt: composed(&selection, &written),
    via: None,
    privileged: true,
    answers: Vec::new(),
    began: now_ms(),
    context: selection.context.clone(),
  };
  let follower = Follower::new(conversations, owner);
  let carried = Carry::new(receiver).carries(&asking, &held, &follower).await;

  Ok(match carried {
    Ok(reply) => Some(crate::form::strip_prompt_scaffolding(&reply.text)),
    Err(failure) => return Err(failure_of(failure)),
  })
}

// The chat agent's conversations take a talk's turns, behind whatever round is running.
impl Talking for Conversations {
  fn talk<'a>(&'a self, request: &'a TalkRequest) -> BoxFuture<'a, Result<Option<String>, TalkFailure>> {
    Box::pin(Conversations::talk(self, request))
  }
}

/// The chat agent's registered label, which its own words are called by.
async fn agent_label(receiver: &Receiver, agent: &str) -> String {
  label_of(receiver, agent).await
}

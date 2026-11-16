//! What the receiver puts into a room from a shell (docs/messages.md): a line, said from a run of its own so
//! nobody waits on the gates; a reaction, a pin, a sticker and pictures; the owner's line that stands without the
//! service; and a procedure document as one room may use it. Each answers the lines it prints.
//!
//! A line of its own is said in a run: its row, its log and its process are opened, written and closed through
//! the launcher seam the binary fills, since the run's rows and its log are the agency's and the receiver holds
//! only the words to be said. With no launcher standing, a line asked to be said is refused.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use bmo_chat_client::{AlbumItem, ChatSend, ChatSendRequest, ChatSticker, FileItem, MissedPictureReason};
use room_calls::procedures;
use room_calls::reading::count_text;
use room_calls::turn::AGENT_VARIABLE as TURN_AGENT;
use serde_json::{Map, Value};

use receiver_codes::RunTitle;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::files::FILES;

use crate::chat::instructions::{Queued, queue};
use crate::data_root::DataRoot;
use crate::receiver::{NotReceived, Receiver, labels, rows, wake};
use crate::verbs::photo_repair::fetched;
use crate::verbs::room_files::{room_file_refusal, room_stores};
use crate::verbs::sends::{Going, NotSent, Sends};
use crate::verbs::{CommandAsked, LaunchAsked, RunAsked, VerbError};
use crate::{Fault, now_ms, one, run};

pub const SAY_USAGE: &str = "usage: bmo room say [--chat <chatId>] [--by chat:<chatId>] [--reply] --text <text>  (the words are read from stdin without --text)";

// How long the room has to answer before the promise lapses.
const RELAY_LIFE_MS: i64 = 30 * 60_000;

// The brake on two rooms answering each other forever: a chat may ask other rooms for at most this many
// answers in this stretch, and the ask beyond it is dropped while the words themselves still go.
const MAX_RELAYS_ASKED: i64 = 3;
const RELAY_CAP_WINDOW_MS: i64 = 10 * 60_000;

// The first line of what was sent is named by at most this many characters.
const OPENING_WIDTH: usize = 60;

/// The unit one line's run goes by. It carries the clock, so a person reading the journal can tell which line a
/// unit was, and a draw of its own beside it, so two lines asked in the same moment never share one.
pub fn say_unit(now_ms: i64) -> String {
  format!("bmo-say-{now_ms}-{:06x}", rand::random::<u32>() & 0x00ff_ffff)
}

/// Which chat is told what became of a line, or none: only a chat session, and only about a room that is not
/// its own.
pub fn hand_back_to(started_by: Option<&str>, chat_id: i64) -> Option<i64> {
  let caller: i64 = started_by?.trim().strip_prefix("chat:")?.parse().ok()?;

  (caller != chat_id).then_some(caller)
}

/// One line asked to be said.
pub struct SayAsked {
  pub chat_id: i64,
  pub text: String,
  /// Whether an answer is wanted back, which is asked for rather than assumed.
  pub reply: bool,
  /// The message this line replies to, by its number in the room.
  pub reply_to: Option<i64>,
  /// Who set it off: a chat session names its own chat and is told there what became of the line.
  pub started_by: Option<String>,
}

/// Why a line is turned away before anything is opened, or none for one that may go.
pub fn say_refusal(asked: &SayAsked) -> Option<&'static str> {
  if asked.text.trim().is_empty() {
    return Some("there are no words to send.");
  }

  if asked.reply && hand_back_to(asked.started_by.as_deref(), asked.chat_id).is_none() {
    return Some(
      "an answer comes back to the chat that asked for it, so --reply needs --by chat:<your own chatId>, and that chat cannot be the room being written to.",
    );
  }

  None
}

/// The first line of what was sent, short enough to name the message without repeating it.
pub fn opening(text: &str) -> String {
  let first = text.trim().split('\n').next().unwrap_or_default();

  if first.chars().count() <= OPENING_WIDTH {
    first.to_owned()
  } else {
    format!("{}…", first.chars().take(OPENING_WIDTH).collect::<String>())
  }
}

/// Whether an answer asked for will be brought back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relay {
  Waiting,
  Dropped,
}

/// What a session is told when its words reached the other room.
pub fn sent_instruction(chat_id: i64, text: &str, reply: Option<Relay>) -> String {
  let mut lines = vec![format!(
    "The message you sent to chat {chat_id} has gone out and is in that room now. First line: {}",
    opening(text),
  )];

  match reply {
    Some(Relay::Waiting) => lines.extend([
      "You asked that room for an answer, so its answer will reach you here on its own as another note like this one. Do not go looking for it and do not send again while you wait; if none comes, that room had nothing to say.",
      "Nothing is needed of you now. Say here that it went only if somebody in this chat is waiting to hear that much, and then in one short line.",
    ].map(str::to_owned)),
    Some(Relay::Dropped) => lines.extend([
      "You asked that room for an answer and this one will not be brought back: you have asked other rooms for several answers in a short stretch, which is the shape of two rooms talking to each other in a circle, so the asking is off for a while. The words themselves went.",
      "Say plainly in this chat that no answer is coming back from that room this time, if anybody here was waiting on one.",
    ].map(str::to_owned)),
    None => lines.push(
      "Nothing is needed of you for it. Say so here only if somebody in this chat is waiting to hear that it went, and then in one short line.".to_owned(),
    ),
  }

  lines.join("\n")
}

/// What a session is told when its words did not go out.
pub fn failed_instruction(chat_id: i64, text: &str, reason: &str) -> String {
  [
    format!("The message you sent to chat {chat_id} did not go out, and nothing more is coming from it. Reason: {reason}"),
    format!("First line of what did not go: {}", opening(text)),
    "Say plainly in this chat that it did not reach that room, and never say it did. Send it again only if somebody asks for it.".to_owned(),
  ]
  .join("\n")
}

/// A line put in a room without holding up whoever asked for it: the road asked, the words put on the disk for
/// the run alone, the run's row opened and the run launched, answered the moment it is up.
fn replying(mut request: ChatSendRequest, reply_to: Option<i64>) -> ChatSendRequest {
  request.reply_to_message_id = reply_to;
  request
}

pub async fn say(sends: &Sends, asked: &SayAsked, via: Option<&str>) -> Result<Vec<String>, VerbError> {
  if let Some(refusal) = say_refusal(asked) {
    return Err(VerbError::Refused(format!("say: {refusal}\n{SAY_USAGE}")));
  }

  // The road is asked first, and a road that does not answer within the bound refuses the line, so the call
  // answers at once either way.
  sends.line_road(asked.chat_id).await?;

  let unit = say_unit(now_ms());
  let text_path = std::env::temp_dir().join(format!("{unit}.txt"));

  write_private(&text_path, asked.text.trim())
    .map_err(|why| VerbError::Refused(format!("the words could not be put on the disk: {why}")))?;

  // A line that answers a named message is said by the receiver itself, not through a runner, so the reply target
  // the tool carries reaches the room with it.
  if asked.reply_to.is_some() {
    let run = SayRun {
      chat_id: asked.chat_id,
      text_path,
      run_id: None,
      unit: unit.clone(),
      started_by: asked.started_by.clone().unwrap_or_default(),
      reply: asked.reply,
      reply_to: asked.reply_to,
      via: via.map(str::to_owned),
    };

    said_apart(sends, run);

    return Ok(vec![launched_line(&unit, asked.chat_id)]);
  }

  let run_id = sends
    .launch()
    .start(&RunAsked {
      agent: "say".to_owned(),
      chat_id: asked.chat_id,
      unit: unit.clone(),
      started_by: asked.started_by.clone(),
      asked_by: sends.turn_agent().map(str::to_owned),
    })
    .await;
  let room = rows::chat_title(&sends.db(), asked.chat_id)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| asked.chat_id.to_string());
  let title = serde_json::to_string(&RunTitle::Say { room }).unwrap_or_default();
  let mut args = vec![
    "say-runner".to_owned(),
    asked.chat_id.to_string(),
    text_path.to_string_lossy().into_owned(),
    run_id.map(|id| id.to_string()).unwrap_or_default(),
    unit.clone(),
    asked.started_by.clone().unwrap_or_default(),
    if asked.reply { "reply".to_owned() } else { String::new() },
  ];

  if let Some(via) = via {
    args.extend(["--via".to_owned(), via.to_owned()]);
  }

  let command = CommandAsked {
    name: "say-runner".to_owned(),
    args,
    env: sends
      .turn_agent()
      .map(|agent| vec![(TURN_AGENT.to_owned(), agent.to_owned())])
      .unwrap_or_default(),
  };
  let launched = sends
    .launch()
    .launch(&LaunchAsked {
      run_id,
      unit: unit.clone(),
      title,
      agent: "say".to_owned(),
      command,
      scratch: vec![text_path.clone()],
      utility: true,
    })
    .await;

  // With no launcher standing, the receiver says the line itself, apart from the call, as the runner would.
  if !launched {
    let run = SayRun {
      chat_id: asked.chat_id,
      text_path,
      run_id,
      unit: unit.clone(),
      started_by: asked.started_by.clone().unwrap_or_default(),
      reply: asked.reply,
      reply_to: asked.reply_to,
      via: via.map(str::to_owned),
    };

    said_apart(sends, run);
  }

  Ok(vec![launched_line(&unit, asked.chat_id)])
}

// A line said by the receiver itself, apart from the call that asked it, as a runner would say it.
fn said_apart(sends: &Sends, run: SayRun) {
  let sends = sends.clone();

  tokio::spawn(async move {
    if let Err(why) = say_run(&sends, &run).await {
      eprintln!("say: chat {}: {why}", run.chat_id);
    }
  });
}

// What the call answers once a line is handed on: the send's id, the unit its run goes by.
fn launched_line(unit: &str, chat_id: i64) -> String {
  format!(
    "say launched as {unit}; it puts the words in chat {chat_id} by itself and says so when it is done. Do not wait for it and do not send them again."
  )
}

// Owner-only from the moment the file exists: these are somebody's own words.
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
  use std::os::unix::fs::OpenOptionsExt as _;

  std::fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .mode(0o600)
    .open(path)?
    .write_all(text.as_bytes())
}

/// One line's run as its launcher handed it over.
pub struct SayRun {
  pub chat_id: i64,
  pub text_path: PathBuf,
  /// The row the launcher opened, or none where it opened none.
  pub run_id: Option<i64>,
  /// The unit the run is, which its log lines are grouped under; empty for no log.
  pub unit: String,
  /// The chat that set it off, empty where nobody named one.
  pub started_by: String,
  /// Whether that chat asked the room for an answer back.
  pub reply: bool,
  /// The message this line replies to, by its number in the room.
  pub reply_to: Option<i64>,
  /// The road the caller's turn is read on, which the news of the line goes back on.
  pub via: Option<String>,
}

/// The half of a line that says it, in its own run: the words through the room's gates and into the room, the
/// row closed with how it ended, the room's window opened, and whoever asked told what became of them. A
/// stop from outside and gates that never finish end it the same way a refusal does.
pub async fn say_run(sends: &Sends, run_: &SayRun) -> Result<Vec<String>, VerbError> {
  let text = std::fs::read_to_string(&run_.text_path)
    .map_err(|why| VerbError::Refused(format!("{}: {why}", run_.text_path.display())))?
    .trim()
    .to_owned();
  let started_by = (!run_.started_by.is_empty()).then_some(run_.started_by.as_str());
  let teller = hand_back_to(started_by, run_.chat_id);
  let length = count_text(text.encode_utf16().count());
  let chat = run_.chat_id.to_string();

  logged(sends, &run_.unit, "start", format!("started: {length} characters to chat {chat}")).await;

  let said = async {
    let road = sends.road(run_.chat_id, "text").await.map_err(|why| why.to_string())?;

    sends
      .send(
        &road,
        &replying(
          ChatSendRequest::new(
            run_.chat_id,
            // A line's words go out as any message does: no send path fixes their form, so no fold is
            // asked here.
            ChatSend::Text {
              text: text.clone(),
              silent: false,
              fold: false,
            },
          ),
          run_.reply_to,
        ),
      )
      .await
      .map(drop)
      .map_err(|why| match why {
        NotSent::Refused(why) | NotSent::Failed(why) => why,
      })
  };

  if let Err(reason) = stopped_or(said).await {
    logged(sends, &run_.unit, "end", format!("failed: {reason}")).await;

    if let Some(teller) = teller {
      let speaker = match sends.turn_agent() {
        Some(agent) => agent.to_owned(),
        None => sends.receiver().default_answerer(),
      };
      let note = failed_instruction(run_.chat_id, &text, &reason);
      let queued = Queued {
        sort: Some("say"),
        via: run_.via.as_deref(),
        ..Queued::default()
      };

      if let Err(why) = queue(&sends.db(), &speaker, teller, &note, &queued).await {
        eprintln!("say: chat {teller} could not be told: {}", why.detail);
      }
    }

    let _ = std::fs::remove_file(&run_.text_path);

    if let Some(run_id) = run_.run_id {
      sends.launch().close(run_id, "failed").await;
    }

    return Err(VerbError::Refused(format!("say run failed: {reason}")));
  }

  // The room has been spoken to, so for the next few minutes an answer in it needs no name to reach the
  // session of the agent that said the line, as if it had said it in the room itself: the agent whose turn set
  // the run going, the room's own voice where none did.
  let asker = match run_.run_id {
    Some(id) => sends.launch().asked_by(id).await,
    None => None,
  };
  let speaker = asker.unwrap_or(
    wake::agents::voice_of(&sends.receiver(), run_.chat_id)
      .await
      .unwrap_or(sends.receiver().default_answerer()),
  );
  let window = window_seconds(&sends.receiver(), &speaker).await;
  let until = now_ms() + window * 1000;

  wake::agents::open_window(&sends.db(), run_.chat_id, &speaker, until)
    .await
    .map_err(unheard)?;
  sends.receiver().news().window(run_.chat_id);
  wake::agents::watch_window(&sends.receiver(), run_.chat_id, &speaker, until);

  let relay = arm_relay(sends, &run_.unit, run_.chat_id, teller, run_.reply, &text).await?;

  if let Some(teller) = teller {
    let note = sent_instruction(run_.chat_id, &text, relay);
    let queued = Queued {
      sort: Some("say"),
      via: run_.via.as_deref(),
      ..Queued::default()
    };

    if let Err(why) = queue(&sends.db(), &speaker, teller, &note, &queued).await {
      eprintln!("say: chat {teller} could not be told: {}", why.detail);
    }
  }

  let _ = std::fs::remove_file(&run_.text_path);
  logged(sends, &run_.unit, "end", format!("sent {length} characters to chat {chat}")).await;

  if let Some(run_id) = run_.run_id {
    sends.launch().close(run_id, "done").await;
  }

  Ok(vec![format!("say: sent {length} characters to chat {}", run_.chat_id)])
}

// A line of the run's log, written under its unit; nothing is written where the run has no unit.
async fn logged(sends: &Sends, unit: &str, kind: &str, words: String) {
  if unit.is_empty() {
    return;
  }

  sends.launch().log(unit, kind, &words).await;
}

// The sending raced against a stop from outside: how it ended, or why it did not. The gates take as long as they
// take; the run ends when they finish or when it is stopped.
async fn stopped_or(said: impl std::future::Future<Output = Result<(), String>>) -> Result<(), String> {
  use tokio::signal::unix::{SignalKind, signal};

  let mut term = signal(SignalKind::terminate()).map_err(|why| why.to_string())?;
  let mut interrupt = signal(SignalKind::interrupt()).map_err(|why| why.to_string())?;

  tokio::select! {
    said = said => said,
    _ = term.recv() => Err("stopped by SIGTERM".to_owned()),
    _ = interrupt.recv() => Err("stopped by SIGINT".to_owned()),
  }
}

// How long an agent's window runs, in seconds, as its registration gives it and the round's own length where
// it gives none.
async fn window_seconds(receiver: &Receiver, agent: &str) -> i64 {
  let listed = receiver.agents().await.ok();

  listed
    .as_ref()
    .and_then(|listed| {
      listed
        .agents
        .iter()
        .find(|one| one.name == agent)
        .and_then(|one| one.row.get("windowSeconds").and_then(|held| held.as_i64()))
    })
    .unwrap_or(CHAT_ROUND.conversation.window_seconds)
}

// The promise of an answer, left where the room's own next answer will find it, armed only once the words
// are in the room.
async fn arm_relay(
  sends: &Sends,
  unit: &str,
  chat_id: i64,
  teller: Option<i64>,
  reply: bool,
  text: &str,
) -> Result<Option<Relay>, VerbError> {
  let Some(teller) = teller.filter(|_| reply) else { return Ok(None) };
  let since = now_ms() - RELAY_CAP_WINDOW_MS;
  let asked: i64 = one(
    &sends.db(),
    r#"SELECT COUNT(*) AS "n" FROM "say_relays" WHERE "reply_to" = $1 AND "created_at" >= $2"#,
    vec![teller.into(), since.into()],
  )
  .await?
  .map(|row| row.try_get("", "n"))
  .transpose()
  .map_err(Fault::from)?
  .unwrap_or(0);

  if asked >= MAX_RELAYS_ASKED {
    logged(
      sends,
      unit,
      "note",
      format!(
        "chat {teller} has asked {asked} rooms for an answer in this stretch, so this one is dropped"
      ),
    )
    .await;

    return Ok(Some(Relay::Dropped));
  }

  run(
    &sends.db(),
    r#"INSERT INTO "say_relays" ("chat_id", "reply_to", "opening", "created_at", "expires_at") VALUES ($1, $2, $3, $4, $5)"#,
    vec![
      chat_id.into(),
      teller.into(),
      opening(text).into(),
      now_ms().into(),
      (now_ms() + RELAY_LIFE_MS).into(),
    ],
  )
  .await.map_err(store)?;

  Ok(Some(Relay::Waiting))
}

fn refused(why: NotSent) -> VerbError {
  VerbError::Refused(why.to_string())
}

fn unheard(why: NotReceived) -> VerbError {
  VerbError::Refused(why.to_string())
}

fn store(why: Fault) -> VerbError {
  VerbError::Store(why)
}

/// A message pinned in a room, quietly, or unpinned.
pub async fn pin(
  sends: &Sends,
  chat_id: i64,
  message_id: i64,
  unpin: bool,
) -> Result<Vec<String>, VerbError> {
  let road = sends.road(chat_id, "pin").await?;

  sends
    .send(
      &road,
      &ChatSendRequest::new(
        chat_id,
        ChatSend::Pin {
          message_id,
          on: !unpin,
          silent: true,
        },
      ),
    )
    .await
    .map_err(refused)?;

  Ok(vec![if unpin {
    format!("unpinned message {message_id}")
  } else {
    format!("pinned message {message_id}")
  }])
}

/// An emoji reaction put on a message, or the agent's own taken off with `none`.
pub async fn react(
  sends: &Sends,
  chat_id: i64,
  message_id: i64,
  emoji: &str,
) -> Result<Vec<String>, VerbError> {
  let emoji = if emoji == "none" { String::new() } else { emoji.to_owned() };
  let road = sends.road(chat_id, "reaction").await?;

  sends
    .send(
      &road,
      &ChatSendRequest::new(
        chat_id,
        ChatSend::Reaction {
          message_id,
          emoji: emoji.clone(),
        },
      ),
    )
    .await
    .map_err(refused)?;

  Ok(vec![if emoji.is_empty() {
    format!("reaction cleared on message {message_id}")
  } else {
    format!("reacted {emoji} to message {message_id}")
  }])
}

/// One of the deployment's stickers, sent by its emoji.
pub async fn sticker(
  sends: &Sends,
  chat_id: i64,
  emoji: &str,
  reply_to: Option<i64>,
) -> Result<Vec<String>, VerbError> {
  let road = sends.road(chat_id, "sticker").await?;

  match sends
    .send(
      &road,
      &replying(
        ChatSendRequest::new(
          chat_id,
          ChatSend::Sticker(ChatSticker::Emoji {
            emoji: emoji.to_owned(),
          }),
        ),
        reply_to,
      ),
    )
    .await
  {
    Ok(_) => Ok(vec![format!("sent the {emoji} sticker to chat {chat_id}")]),
    Err(NotSent::Refused(refusal)) => {
      Err(VerbError::Refused(format!("{refusal} (--list shows the stickers there are)")))
    }
    Err(NotSent::Failed(why)) => Err(VerbError::Refused(why)),
  }
}

/// The set the deployment's stickers are sent out of: the set belongs to the chat service, which says what it
/// carries in its own words, so the listing names the road rather than a set of our own.
pub fn sticker_list() -> Result<Vec<String>, VerbError> {
  Ok(vec!["the stickers there are belong to the road the room is answered on, which says in its own words when a sticker is not one of them".to_owned()])
}

/// One picture asked for: a web address or a file this machine keeps, with the word drawn into it where one
/// is named.
#[derive(Clone, Debug)]
pub struct Picture {
  pub url: String,
  pub label: Option<String>,
}

/// One picture as it stands before a road carries it: a web address the road fetches for itself, or the bytes of
/// a file under the data root, read here so the road needs no reach into this machine's data root.
enum PictureSource {
  Web(String),
  Kept {
    name: String,
    mime: String,
    bytes: Vec<u8>,
  },
}

/// One picture, or several as one album: none that a room keeps goes into another room or out of a job, the
/// road is asked before anything is uploaded, a label is drawn in where one is named, and what did not go is
/// said by address with why. A web address goes as it stands; a file under the data root is read here and goes
/// as its bytes, since no road reaches this machine's data root. Anything else is refused before anything goes.
pub async fn photo(
  sends: &Sends,
  data_root: &DataRoot,
  chat_id: i64,
  pictures: &[Picture],
  caption: Option<&str>,
  reply_to: Option<i64>,
  job: bool,
) -> Result<Vec<String>, VerbError> {
  let pictures: Vec<(String, String)> = pictures
    .iter()
    .map(|picture| {
      (
        picture.url.trim().to_owned(),
        picture.label.as_deref().unwrap_or_default().trim().to_owned(),
      )
    })
    .filter(|(url, _)| !url.is_empty())
    .collect();

  if pictures.is_empty() {
    return Err(VerbError::Refused("a picture to send and the room to send it to".to_owned()));
  }

  let stores = room_stores(data_root);

  if let Some(refusal) = pictures.iter().find_map(|(url, _)| room_file_refusal(url, chat_id, &stores, job))
  {
    return Err(VerbError::Refused(refusal));
  }

  let road = sends.road(chat_id, "album").await?;
  let mut unlabelled: Vec<String> = vec![];
  let mut items: Vec<AlbumItem> = vec![];

  for (url, label) in &pictures {
    let source = picture_source(data_root, url).await?;

    // The path a relative address resolved to may stand in another room's store, which the first check could
    // not read from the address as it was written.
    if let PictureSource::Kept { .. } = &source
      && let Some(refusal) = resolved_room_refusal(data_root, url, chat_id, &stores, job)
    {
      return Err(VerbError::Refused(refusal));
    }

    if !label.is_empty() {
      if let Some(item) = labelled_item(&source, label).await {
        items.push(AlbumItem::File(item));
        continue;
      }

      unlabelled.push(url.clone());
    }

    items.push(match source {
      PictureSource::Web(address) => AlbumItem::Address { address },
      PictureSource::Kept { name, mime, bytes } => AlbumItem::File(FileItem { name, mime, bytes }),
    });
  }

  let sent = sends
    .send(
      &road,
      &replying(
        ChatSendRequest::new(
          chat_id,
          ChatSend::Album {
            items,
            caption: caption.map(str::to_owned),
          },
        ),
        reply_to,
      ),
    )
    .await
    .map_err(refused)?;
  let mut lines: Vec<String> = vec![];

  if !sent.missed.is_empty() {
    let missed: Vec<String> = sent
      .missed
      .iter()
      .map(|picture| format!("{} — {}", picture.address, picture.reason))
      .collect();

    lines.push(format!("not sent as pictures ({}):\n{}", sent.missed.len(), missed.join("\n")));
  }

  if !unlabelled.is_empty() {
    lines.push(format!(
      "sent without the label drawn in ({}):\n{}",
      unlabelled.len(),
      unlabelled.join("\n")
    ));
  }

  if sent.missed.len() == pictures.len() {
    return Err(VerbError::Partly {
      lines,
      refusal: "no picture was sent".to_owned(),
    });
  }

  lines.push(if pictures.len() == 1 {
    "sent".to_owned()
  } else {
    format!("sent {} of {} pictures as an album", pictures.len() - sent.missed.len(), pictures.len())
  });

  Ok(lines)
}

/// One file of a file send: the path it stands at, and the name the room reads it under where one is named.
#[derive(Clone, Debug)]
pub struct SendFile {
  pub path: String,
  pub name: Option<String>,
}

// One file read off this machine for a send: the name the room reads, its type and its bytes.
struct ReadFile {
  name: String,
  mime: String,
  bytes: Vec<u8>,
}

// A file read for a send, held to the ceiling one file may be, refused where it is past it or is no file; a
// file a room's folder holds goes only into that room and out of no job.
async fn read_file(
  data_root: &DataRoot,
  chat_id: i64,
  path: &str,
  name: Option<&str>,
  job: bool,
) -> Result<ReadFile, VerbError> {
  if let Some(refusal) = room_file_refusal(path, chat_id, &room_stores(data_root), job) {
    return Err(VerbError::Refused(refusal));
  }

  let length = tokio::fs::metadata(path)
    .await
    .map_err(|why| VerbError::Refused(format!("{path} is no file on this machine: {why}")))?
    .len();

  if length > FILES.largest_bytes {
    return Err(VerbError::Refused(format!(
      "{path} is {length} bytes, past the {} a send takes a file at",
      FILES.largest_bytes
    )));
  }

  let bytes = tokio::fs::read(path)
    .await
    .map_err(|why| VerbError::Refused(format!("{path} could not be read: {why}")))?;
  let named = name
    .map(str::trim)
    .filter(|name| !name.is_empty())
    .and_then(|name| Path::new(name).file_name())
    .or_else(|| Path::new(path).file_name())
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_else(|| FILES.unnamed.clone());
  let mime = mime_guess::from_path(&named)
    .first()
    .map(|kind| kind.essence_str().to_owned())
    .unwrap_or_else(|| "application/octet-stream".to_owned());

  Ok(ReadFile {
    name: named,
    mime,
    bytes,
  })
}

/// One file or several: one goes as a document, several as one album, each read from this machine. A caption goes
/// on an album, and ahead of a document as a line of its own, since the document kind carries none.
pub async fn file(
  sends: &Sends,
  data_root: &DataRoot,
  chat_id: i64,
  files: &[SendFile],
  caption: Option<&str>,
  reply_to: Option<i64>,
  job: bool,
) -> Result<Vec<String>, VerbError> {
  let mut held: Vec<ReadFile> = vec![];

  for file in files {
    let path = file.path.trim();

    if !path.is_empty() {
      held.push(read_file(data_root, chat_id, path, file.name.as_deref(), job).await?);
    }
  }

  if held.is_empty() {
    return Err(VerbError::Refused("a file to send and the room to send it to".to_owned()));
  }

  let caption = caption.map(str::trim).filter(|caption| !caption.is_empty());
  let mut lines: Vec<String> = vec![];

  if held.len() == 1 {
    let file = held.remove(0);
    let road = sends.road(chat_id, "document").await?;

    if let Some(caption) = caption {
      sends
        .send(
          &road,
          &replying(
            ChatSendRequest::new(
              chat_id,
              ChatSend::Text {
                text: caption.to_owned(),
                silent: false,
                fold: false,
              },
            ),
            reply_to,
          ),
        )
        .await
        .map_err(refused)?;
      lines.push("said the caption".to_owned());
    }

    let sent = sends
      .send(
        &road,
        &replying(
          ChatSendRequest::new(
            chat_id,
            ChatSend::Document {
              bytes: file.bytes,
              file_name: file.name.clone(),
            },
          ),
          reply_to,
        ),
      )
      .await;

    match sent {
      Ok(_) => lines.push(format!("sent {} to chat {chat_id}", file.name)),
      Err(why) if lines.is_empty() => return Err(refused(why)),
      Err(why) => {
        return Err(VerbError::Partly {
          lines,
          refusal: why.to_string(),
        });
      }
    }
  } else {
    let road = sends.road(chat_id, "album").await?;
    let count = held.len();
    let items: Vec<AlbumItem> = held
      .into_iter()
      .map(|file| {
        AlbumItem::File(FileItem {
          name: file.name,
          mime: file.mime,
          bytes: file.bytes,
        })
      })
      .collect();
    let sent = sends
      .send(
        &road,
        &replying(
          ChatSendRequest::new(
            chat_id,
            ChatSend::Album {
              items,
              caption: caption.map(str::to_owned),
            },
          ),
          reply_to,
        ),
      )
      .await
      .map_err(refused)?;

    if !sent.missed.is_empty() {
      let missed: Vec<String> =
        sent.missed.iter().map(|one| format!("{} — {}", one.address, one.reason)).collect();

      lines.push(format!("not sent as files ({}):\n{}", sent.missed.len(), missed.join("\n")));
    }

    lines.push(format!(
      "sent {} of {count} files as an album to chat {chat_id}",
      count - sent.missed.len()
    ));
  }

  Ok(lines)
}

/// A voice note asked of a room: the file's path, its caption, what it says and what was read out.
#[derive(Clone, Copy, Debug, Default)]
pub struct VoiceNote<'a> {
  pub path: &'a str,
  pub caption: Option<&'a str>,
  pub said: Option<&'a str>,
  pub transcript: Option<&'a str>,
}

/// A voice note: an OGG/Opus file read from this machine, with its caption and what it says, sent as a voice
/// message the room can play.
pub async fn voice(
  sends: &Sends,
  data_root: &DataRoot,
  chat_id: i64,
  note: VoiceNote<'_>,
  reply_to: Option<i64>,
  job: bool,
) -> Result<Vec<String>, VerbError> {
  let path = note.path.trim();
  let file = read_file(data_root, chat_id, path, None, job).await?;

  if !file.bytes.starts_with(b"OggS") {
    return Err(VerbError::Refused(format!(
      "{path} is not an OGG/Opus file, so it is not sent as a voice note"
    )));
  }

  let caption = note.caption.map(str::trim).filter(|caption| !caption.is_empty()).map(str::to_owned);
  let transcript = note
    .transcript
    .map(str::trim)
    .filter(|transcript| !transcript.is_empty())
    .map(str::to_owned);
  let said = note
    .said
    .map(str::trim)
    .filter(|said| !said.is_empty())
    .map(str::to_owned)
    .or_else(|| transcript.clone())
    .or_else(|| caption.clone())
    .or_else(|| Path::new(&file.name).file_stem().map(|stem| stem.to_string_lossy().into_owned()))
    .unwrap_or_else(|| "voice".to_owned());
  let road = sends.road(chat_id, "voice").await?;

  sends
    .send(
      &road,
      &replying(
        ChatSendRequest::new(
          chat_id,
          ChatSend::Voice {
            audio: file.bytes,
            said,
            caption,
            transcript,
          },
        ),
        reply_to,
      ),
    )
    .await
    .map_err(refused)?;

  Ok(vec![format!("sent the voice note {} to chat {chat_id}", file.name)])
}

/// A fixed line: its code and fields rendered by the web unit's language packs, said into the room without the
/// output gate or any session, since the line is already read.
pub async fn fixed_line(
  sends: &Sends,
  chat_id: i64,
  code: &str,
  fields: &Map<String, Value>,
) -> Result<Vec<String>, VerbError> {
  let code = code.trim();

  if code.is_empty() {
    return Err(VerbError::Refused("a fixed line's code to send".to_owned()));
  }

  let road = sends.road(chat_id, "text").await?;
  let Some(text) = sends.speech().rendered_code(code, fields.clone()).await else {
    return Err(VerbError::Refused(format!(
      "no sentence in the rooms' language can be made of the fixed line {code}, so nothing was said"
    )));
  };

  sends
    .send(
      &road,
      &ChatSendRequest::new(
        chat_id,
        ChatSend::Text {
          text,
          silent: false,
          fold: false,
        },
      ),
    )
    .await
    .map_err(refused)?;

  Ok(vec![format!("said the fixed line {code} in chat {chat_id}")])
}

// One picture resolved: a web address as it stands, or a file under the data root read into bytes. An address
// that is neither is refused with the reason the pictures policy names.
async fn picture_source(data_root: &DataRoot, address: &str) -> Result<PictureSource, VerbError> {
  if crate::web_address::is_web_address(address) {
    return Ok(PictureSource::Web(address.to_owned()));
  }

  let path =
    kept_path(data_root, address).map_err(|reason| VerbError::Refused(format!("{address}: {reason}")))?;
  let bytes = tokio::fs::read(&path)
    .await
    .map_err(|why| VerbError::Refused(format!("{address} could not be read: {why}")))?;
  let name = path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .filter(|name| !name.trim().is_empty())
    .unwrap_or_else(|| FILES.unnamed.clone());
  let mime = mime_guess::from_path(&name)
    .first()
    .map(|kind| kind.essence_str().to_owned())
    .unwrap_or_else(|| "application/octet-stream".to_owned());

  Ok(PictureSource::Kept { name, mime, bytes })
}

// Where the file an address names stands under the data root, every link followed, or why it may not be sent.
// A relative path is read against the folder that holds the data root, then the data root itself, then the
// working folder, so `data/collected/...` names the same file however the receiver was started.
fn kept_path(data_root: &DataRoot, address: &str) -> Result<PathBuf, MissedPictureReason> {
  let path = Path::new(address);
  let root = data_root.path().canonicalize().unwrap_or_else(|_| absolute_lexical(data_root.path()));
  let candidates: Vec<PathBuf> = if path.is_absolute() {
    vec![path.to_path_buf()]
  } else {
    let mut candidates = vec![];

    if let Some(parent) = data_root.path().parent() {
      candidates.push(parent.join(path));
    }

    candidates.push(data_root.path().join(path));

    if let Ok(cwd) = std::env::current_dir() {
      candidates.push(cwd.join(path));
    }

    candidates
  };

  for candidate in candidates {
    match candidate.canonicalize() {
      Ok(real) if real.starts_with(&root) => {
        return if real.is_file() { Ok(real) } else { Err(MissedPictureReason::MissingFile) };
      }
      Ok(_) => {}
      Err(_) if absolute_lexical(&candidate).starts_with(&root) => {
        return Err(MissedPictureReason::MissingFile);
      }
      Err(_) => {}
    }
  }

  Err(if path.is_absolute() || address.contains('/') {
    MissedPictureReason::OutsideDataRoot
  } else {
    MissedPictureReason::NotAnAddress
  })
}

fn absolute_lexical(path: &Path) -> PathBuf {
  if path.is_absolute() {
    return path.to_path_buf();
  }

  std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

// The room a resolved file stands in, refused where it is another room's or a job sends it.
fn resolved_room_refusal(
  data_root: &DataRoot,
  address: &str,
  chat_id: i64,
  stores: &[PathBuf],
  job: bool,
) -> Option<String> {
  let path = kept_path(data_root, address).ok()?;

  room_file_refusal(&path.to_string_lossy(), chat_id, stores, job)
}

// A picture with its word drawn into it, as the JPEG bytes the road carries. None where it could not be read,
// was no picture, or the label said nothing, so the caller sends the picture as it stands instead.
async fn labelled_item(source: &PictureSource, label: &str) -> Option<FileItem> {
  let bytes = match source {
    PictureSource::Web(address) => fetched(address).await.ok()?,
    PictureSource::Kept { bytes, .. } => bytes.clone(),
  };

  if bytes.is_empty() {
    return None;
  }

  let wording = label.to_owned();
  let drawn = tokio::task::spawn_blocking(move || labels::labelled(&bytes, &wording)).await.ok()?;
  let jpeg = drawn.ok()??;

  Some(FileItem {
    name: labelled_name(source),
    mime: "image/jpeg".to_owned(),
    bytes: jpeg,
  })
}

fn labelled_name(source: &PictureSource) -> String {
  let name = match source {
    PictureSource::Web(address) => address.rsplit('/').next().unwrap_or_default().to_owned(),
    PictureSource::Kept { name, .. } => name.clone(),
  };
  let stem = Path::new(&name)
    .file_stem()
    .map(|stem| stem.to_string_lossy().into_owned())
    .filter(|stem| !stem.trim().is_empty())
    .unwrap_or_else(|| "picture".to_owned());

  format!("{stem}.jpg")
}

/// One line to the owner, word for word and at once, where a line nobody asked for goes in his chat — its
/// record, kept by the receiver's own lines, whose keeping tells the screens that ring his devices — and on
/// the chat's own road where that could not take it: no gate and no session in between, for a sender that
/// cannot count on the receiver being up.
pub async fn tell_owner(sends: &Sends, text: &str) -> Result<Vec<String>, VerbError> {
  let text = text.trim();

  if text.is_empty() {
    return Err(VerbError::Refused(
      "usage: bmo owner tell --text '<words>'  (or the words on stdin)".to_owned(),
    ));
  }

  let send = ChatSend::Text {
    text: text.to_owned(),
    silent: false,
    fold: false,
  };
  // A line to the owner goes into his private room.
  let owner = sends.receiver().owner_room();
  let unasked = sends.own_road(owner, "text").await;
  let unasked_key = unasked.as_ref().map(|going| going.road.key().to_owned()).ok();
  let Err(why) = on(sends, owner, &send, unasked).await else {
    return Ok(vec!["tell-owner: sent".to_owned()]);
  };
  let own = sends.room_road(owner, "text").await;

  // A road that already refused it is not asked again: a line it may have carried after all goes once.
  if own.as_ref().map(|going| going.road.key().to_owned()).ok() == unasked_key {
    return Err(VerbError::Refused(format!("tell-owner: the line was not sent: {why}")));
  }

  match on(sends, owner, &send, own).await {
    Ok(()) => Ok(vec![format!("tell-owner: sent on the chat's own road, its record refused: {why}")]),
    Err(again) => Err(VerbError::Refused(format!("tell-owner: the line was not sent: {why}; {again}"))),
  }
}

// One line carried on a road, or why it did not.
async fn on(
  sends: &Sends,
  owner: i64,
  send: &ChatSend,
  road: Result<Going, VerbError>,
) -> Result<(), String> {
  match road {
    Ok(road) => sends
      .send(&road, &ChatSendRequest::new(owner, send.clone()))
      .await
      .map(drop)
      .map_err(|why| why.to_string()),
    Err(why) => Err(why.to_string()),
  }
}

/// A procedure document as the room it is read in may use it: the sections and lines for a verb that room's
/// road does not perform left out, as `room-calls` holds a document to the verbs. A read made outside any room
/// has no road to hold the document to, and reads it whole. A name that is no procedure is refused rather than
/// read.
pub async fn procedure(
  sends: &Sends,
  tree: &Path,
  name: &str,
  chat_id: Option<i64>,
) -> Result<Vec<String>, VerbError> {
  let text = procedures::written(tree, name).map_err(VerbError::Refused)?;
  let can = match chat_id {
    Some(chat_id) => Some(sends.answering_can(chat_id).await?),
    None => None,
  };

  Ok(vec![procedures::as_used(&text, can.as_deref()).map_err(VerbError::Refused)?])
}

//! What the receiver puts into a room from a shell (docs/messages.md): a line, said from a run of its own so
//! nobody waits on the gates; a reaction, a pin, a sticker and pictures; the owner's line that stands without the
//! service; and a procedure document as one room may use it. Each answers the lines it prints.
//!
//! A line of its own is said in a run: its row, its log and its process are opened, written and closed through
//! the launcher seam the binary fills, since the run's rows and its log are the agency's and the receiver holds
//! only the words to be said. With no launcher standing, a line asked to be said is refused.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use bmo_chat_client::{AlbumItem, ChatSend, ChatSendRequest};
use room_calls::procedures;
use room_calls::reading::count_text;
use room_calls::turn::AGENT_VARIABLE as TURN_AGENT;
use sha2::{Digest, Sha256};

use receiver_codes::RunTitle;
use receiver_policy::chat_round::CHAT_ROUND;

use crate::chat::instructions::{Queued, queue};
use crate::data_root::DataRoot;
use crate::receiver::{NotReceived, Receiver, labels, rows, wake};
use crate::verbs::photo_repair::fetched;
use crate::verbs::room_files::{room_file_refusal, room_stores};
use crate::verbs::sends::{Going, NotSent, Sends};
use crate::verbs::{CommandAsked, LaunchAsked, RunAsked, VerbError};
use crate::{Fault, now_ms, one, run};

/// How long one line's run may take before it is called off. The gates are the whole of the wait, and three
/// passes of the pair is the worst measured.
pub const SAY_CAP_MS: i64 = 5 * 60_000;

pub const SAY_USAGE: &str = "usage: bmo room say [--chat <chatId>] [--by chat:<chatId>] [--reply] --text <text>  (the words are read from stdin without --text)";

// How long the room has to answer before the promise lapses.
const RELAY_LIFE_MS: i64 = 30 * 60_000;

// The brake on two rooms answering each other forever: a chat may ask other rooms for at most this many
// answers in this stretch, and the ask beyond it is dropped while the words themselves still go.
const MAX_RELAYS_ASKED: i64 = 3;
const RELAY_CAP_WINDOW_MS: i64 = 10 * 60_000;

// The first line of what was sent is named by at most this many characters.
const OPENING_WIDTH: usize = 60;

/// The unit one line's run goes by. It carries the clock, so two never collide and a person reading the
/// journal can tell which line a unit was.
pub fn say_unit(now_ms: i64) -> String {
  format!("bmo-say-{now_ms}")
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
pub async fn say(sends: &Sends, asked: &SayAsked, via: Option<&str>) -> Result<Vec<String>, VerbError> {
  if let Some(refusal) = say_refusal(asked) {
    return Err(VerbError::Refused(format!("say: {refusal}\n{SAY_USAGE}")));
  }

  sends.road(asked.chat_id, "text").await?;

  let unit = say_unit(now_ms());
  let text_path = std::env::temp_dir().join(format!("{unit}.txt"));

  write_private(&text_path, asked.text.trim())
    .map_err(|why| VerbError::Refused(format!("the words could not be put on the disk: {why}")))?;

  let run_id = sends
    .launch()
    .start(&RunAsked {
      agent: "say".to_owned(),
      chat_id: asked.chat_id,
      unit: unit.clone(),
      cap_ms: Some(SAY_CAP_MS),
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
      scratch: vec![text_path],
      utility: true,
    })
    .await;

  if !launched {
    return Err(VerbError::Refused(format!("{unit} did not come up")));
  }

  Ok(vec![format!(
    "say launched as {unit}; it puts the words in chat {} by itself and says so when it is done. Do not wait for it and do not send them again.",
    asked.chat_id
  )])
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
        &ChatSendRequest::new(
          run_.chat_id,
          ChatSend::Text {
            text: text.clone(),
            reply_to: None,
            silent: false,
            fold: true,
          },
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
        None => sends.receiver().deployment().agent.clone(),
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
      .unwrap_or(sends.receiver().deployment().agent.clone()),
  );
  let window = window_seconds(&sends.receiver(), &speaker).await;
  let until = (now_ms() + window * 1000) / 1000;

  wake::agents::open_window(&sends.db(), run_.chat_id, &speaker, until)
    .await
    .map_err(unheard)?;
  sends.receiver().news().window(run_.chat_id);

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

// The sending raced against a stop from outside and the cap: how it ended, or why it did not.
async fn stopped_or(said: impl std::future::Future<Output = Result<(), String>>) -> Result<(), String> {
  use tokio::signal::unix::{SignalKind, signal};

  let mut term = signal(SignalKind::terminate()).map_err(|why| why.to_string())?;
  let mut interrupt = signal(SignalKind::interrupt()).map_err(|why| why.to_string())?;
  let cap = Duration::from_millis(u64::try_from(SAY_CAP_MS).unwrap_or(0));

  tokio::select! {
    said = said => said,
    _ = term.recv() => Err("stopped by SIGTERM".to_owned()),
    _ = interrupt.recv() => Err("stopped by SIGINT".to_owned()),
    () = tokio::time::sleep(cap) => Err(format!("the gates did not finish within {}s", cap.as_secs())),
  }
}

// How long an agent's window runs, in seconds, as its registration gives it and the round's own length where
// it gives none.
async fn window_seconds(receiver: &Receiver, agent: &str) -> i64 {
  let listed = receiver.units().agency.agents().await.ok();

  listed
    .as_ref()
    .and_then(|listed| {
      listed
        .agents
        .iter()
        .find(|one| one.name == agent)
        .and_then(|one| one.row.get("window_seconds").and_then(|held| held.as_i64()))
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
  let since = (now_ms() - RELAY_CAP_WINDOW_MS) / 1000;
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
      (now_ms() / 1000).into(),
      ((now_ms() + RELAY_LIFE_MS) / 1000).into(),
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
pub async fn sticker(sends: &Sends, chat_id: i64, emoji: &str) -> Result<Vec<String>, VerbError> {
  let road = sends.road(chat_id, "sticker").await?;

  match sends
    .send(
      &road,
      &ChatSendRequest::new(
        chat_id,
        ChatSend::Sticker {
          emoji: emoji.to_owned(),
        },
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
pub async fn sticker_list(_sends: &Sends) -> Result<Vec<String>, VerbError> {
  Ok(vec!["the stickers there are belong to the road the room is answered on, which says in its own words when a sticker is not one of them".to_owned()])
}

/// One picture asked for: a web address or a file this machine keeps, with the word drawn into it where one
/// is named.
#[derive(Clone, Debug)]
pub struct Picture {
  pub url: String,
  pub label: Option<String>,
}

/// One picture, or several as one album: none that a room keeps goes into another room or out of a job, the
/// road is asked before anything is uploaded, a label is drawn in where one is named, and what did not go is
/// said by address with why.
pub async fn photo(
  sends: &Sends,
  data_root: &DataRoot,
  chat_id: i64,
  pictures: &[Picture],
  caption: Option<&str>,
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
  let mut sending: Vec<String> = vec![];

  for (url, label) in &pictures {
    if label.is_empty() {
      sending.push(url.clone());
      continue;
    }

    match labelled_file(data_root, url, label).await {
      Some(path) => sending.push(path.to_string_lossy().into_owned()),
      None => {
        unlabelled.push(url.clone());
        sending.push(url.clone());
      }
    }
  }

  let items: Vec<AlbumItem> = sending
    .iter()
    .map(|url| AlbumItem::Address {
      address: url.clone(),
    })
    .collect();
  let sent = sends
    .send(
      &road,
      &ChatSendRequest::new(
        chat_id,
        ChatSend::Album {
          items,
          caption: caption.map(str::to_owned),
          reply_to: None,
        },
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

/// Where labelled pictures are kept: under the data root, since a room is only ever sent a file that stands
/// there. One file per picture-and-label pair, so the same product labelled twice is one file.
pub fn labelled_directory(data_root: &DataRoot) -> PathBuf {
  data_root.path().join("labelled")
}

// A picture named by address or kept file, labelled and written down. None where the picture could not be
// reached or the label said nothing.
async fn labelled_file(data_root: &DataRoot, address: &str, label: &str) -> Option<PathBuf> {
  let bytes = fetched(address).await.ok()?;
  let wording = label.to_owned();
  let drawn = tokio::task::spawn_blocking(move || labels::labelled(&bytes, &wording)).await.ok()?;
  let labelled = drawn.ok()??;
  let digest = Sha256::new().chain_update(address).chain_update("\0").chain_update(label).finalize();
  let name: String = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>()[..32].to_owned();
  let directory = labelled_directory(data_root);
  let path = directory.join(format!("{name}.jpg"));

  std::fs::create_dir_all(&directory).ok()?;
  std::fs::write(&path, labelled).ok()?;

  Some(path)
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
    reply_to: None,
    silent: false,
    fold: false,
  };
  let owner = sends.receiver().owner_id();
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

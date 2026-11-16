//! Words and files typed into a room on the screen (docs/receiver.md, 「The owner's words」, 「Console rooms and
//! accounts」): in every room they are kept under a console number and under who speaks them, the owner on his road
//! or an account on its own, handed to the room's conversations as any words are, and carried to the room's other
//! providers as who said them goes. A room the receiver does not hold is refused before anything is written.

use axum::{
  Json,
  extract::{Multipart, Path, State},
  http::StatusCode,
};
use bmo_chat_client::{ChatFileKind, FileItem, MessageFile};
use receiver_policy::roads::ROADS;
use serde::Deserialize;
use serde_json::{Value, json};

use super::accounts::speaker;
use super::writable::{WritableRoom, Writer};
use super::{Answered, Refusal, Served, answered, read, refused, store};
use crate::data_root::room_path;
use crate::now_ms;
use crate::receiver::{Context, Inbound, NotReceived, Receiver, reads, rows};
use crate::roads::providers::{self, Carried, Delivery, SaidBy};
use crate::roads::{Road, record};
use crate::routes::as_owner::{OwnerFile, refusal};
use crate::routes::message_files::inside;
use crate::secrets;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SayBody {
  chat_id: i64,
  text: String,

  #[serde(default)]
  reply_to: Option<Value>,
}

// A number written as words is read as far as its digits go, the way a browser's parseInt reads one.
fn leading_integer(text: &str) -> Option<i64> {
  let text = text.trim_start();
  let (sign, digits) = match text.strip_prefix('-') {
    Some(rest) => (-1, rest),
    None => (1, text.strip_prefix('+').unwrap_or(text)),
  };
  let digits = digits.chars().take_while(char::is_ascii_digit).collect::<String>();

  digits.parse::<i64>().ok().and_then(|number| number.checked_mul(sign))
}

// Whether the room is one the receiver serves: the owner's own, or one its record holds a title or a message for.
pub(crate) async fn known(receiver: &Receiver, chat_id: i64) -> Result<bool, Refusal> {
  if chat_id == receiver.owner_room() {
    return Ok(true);
  }

  reads::recorded(receiver.db(), chat_id).await.map_err(store)
}

// The message the words answer, where they name one: refused where they name one the room does not hold.
async fn reply_target(
  receiver: &Receiver,
  chat_id: i64,
  value: Option<&Value>,
) -> Result<Option<i64>, Refusal> {
  let number = match value {
    None | Some(Value::Null) => return Ok(None),
    Some(Value::String(text)) if text.is_empty() => return Ok(None),
    Some(Value::Number(number)) => number.as_i64(),
    Some(Value::String(text)) => leading_integer(text),
    Some(other) => leading_integer(&other.to_string()),
  };
  let Some(message_id) = number.filter(|number| *number != 0) else {
    return Err(refused(StatusCode::CONFLICT, "reply-missing", "replyTo must be a message number"));
  };

  if rows::message(receiver.db(), chat_id, message_id, Road::Record.key())
    .await
    .map_err(store)?
    .is_none()
  {
    return Err(refused(
      StatusCode::CONFLICT,
      "reply-missing",
      &format!("the room holds no message {message_id}, so nothing is answered"),
    ));
  }

  Ok(Some(message_id))
}

// Why a call the receiver's own lines needed failed, in the receiver's error shape.
fn failed(why: NotReceived) -> Refusal {
  match why {
    NotReceived::Store(why) => store(why),
    why => refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string()),
  }
}

/// Who words put into a room on the screen stand under: the sender and the name they are kept under, and who said
/// them as the room's other providers are handed them.
pub(super) struct Speaking {
  pub sender_id: i64,
  pub name: String,
  pub said_by: SaidBy,
}

impl Speaking {
  /// The owner, under his number and the name the room shows him by.
  pub async fn owner(receiver: &Receiver, chat_id: i64) -> Self {
    Self {
      sender_id: receiver.owner_id(),
      name: receiver.owner_name(chat_id).await,
      said_by: SaidBy::Owner,
    }
  }
}

/// `POST /owner/messages`: the owner's words, on his own road, which writes in no other account's private room.
pub async fn say(State(served): State<Served>, writer: Writer) -> Answered {
  let asked = read::<SayBody>(writer.body())?;
  let receiver: Receiver = (*served.receiver).clone();

  writer.owners().room(&receiver, asked.chat_id).await?;

  let text = speakable(&receiver, asked.chat_id, &asked.text).await?;
  let speaking = Speaking::owner(&receiver, asked.chat_id).await;
  let id = said(&receiver, asked.chat_id, &text, asked.reply_to.as_ref(), speaking).await?;

  answered(json!({ "id": id }))
}

/// Words to put into a room, their secret-shaped runs taken out: refused where nothing is left, where the receiver
/// does not hold the room, and where the room is archived.
pub(super) async fn speakable(receiver: &Receiver, chat_id: i64, text: &str) -> Result<String, Refusal> {
  let text = secrets::redacted(text.trim());

  if text.is_empty() {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      "chatId (number) and text (non-empty string) are required",
    ));
  }

  if !known(receiver, chat_id).await? {
    return Err(refused(StatusCode::NOT_FOUND, "no-route", &format!("unknown chat {chat_id}")));
  }

  locked(receiver, chat_id).await?;

  Ok(text)
}

/// Words kept in a room under who speaks them, handed to the room's conversations as any words are and carried to
/// the room's other providers: the number they are kept under.
pub(super) async fn said(
  receiver: &Receiver,
  chat_id: i64,
  text: &str,
  reply_to: Option<&Value>,
  speaking: Speaking,
) -> Result<i64, Refusal> {
  let reply = reply_target(receiver, chat_id, reply_to).await?;
  let message_id = record::next_number(receiver.db(), chat_id).await.map_err(store)?;
  let message = Inbound {
    chat_id,
    message_id,
    sender_id: Some(speaking.sender_id),
    sender_name: speaking.name,
    sent_at: now_ms(),
    text: text.to_owned(),
    image_path: None,
    image_state: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
    reply_to_message_id: reply,
    via: Road::Record.key().to_owned(),
    mode: None,
    via_reader: None,
    album: None,
  };

  Receiver::message(receiver, &message, &Context::default(), receiver.roads().room_facts(chat_id).await)
    .await
    .map_err(failed)?;
  carried(receiver, &message, speaking.said_by, Carried::words(text)).await?;

  Ok(message_id)
}

// The words, once the room kept them, carried to the room's other providers as who said them goes (docs/receiver.md,
// 「A room's chat providers」); words the room did not keep go nowhere.
async fn carried(
  receiver: &Receiver,
  kept: &Inbound,
  said_by: SaidBy,
  carried: Carried,
) -> Result<(), Refusal> {
  let held = rows::message(receiver.db(), kept.chat_id, kept.message_id, &kept.via)
    .await
    .map_err(store)?;

  if held.is_none() {
    return Ok(());
  }

  let delivery = Delivery {
    chat_id: kept.chat_id,
    message_id: kept.message_id,
    reply_to: kept.reply_to_message_id,
    said_by,
    standing: vec![kept.via.clone()],
    carried,
  };

  providers::deliver(receiver.db(), receiver.roads(), &delivery).await;

  Ok(())
}

/// A form of files put into a room on the screen, as it was read: its room and the account speaking where the form
/// names them, the caption, the message it answers, whether it is a voice note, and the files.
#[derive(Default)]
pub(super) struct FilesForm {
  pub chat_id: Option<i64>,
  pub account: Option<i64>,
  pub name: Option<String>,
  caption: String,
  reply: Option<String>,
  voice: bool,
  given: Vec<OwnerFile>,
}

fn unreadable(why: axum::extract::multipart::MultipartError) -> Refusal {
  refused(StatusCode::BAD_REQUEST, "bad-request", &why.body_text())
}

/// The form read whole.
pub(super) async fn files_form(mut form: Multipart) -> Result<FilesForm, Refusal> {
  let mut read = FilesForm::default();

  while let Some(field) = form.next_field().await.map_err(unreadable)? {
    let name = field.name().unwrap_or_default().to_owned();

    match name.as_str() {
      "chatId" => read.chat_id = leading_integer(&field.text().await.map_err(unreadable)?),
      "account" => read.account = leading_integer(&field.text().await.map_err(unreadable)?),
      "name" => read.name = Some(field.text().await.map_err(unreadable)?),
      "text" | "caption" => read.caption = field.text().await.map_err(unreadable)?,
      "replyTo" => read.reply = Some(field.text().await.map_err(unreadable)?),
      "voice" => {
        let written = field.text().await.map_err(unreadable)?;

        read.voice = matches!(written.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on");
      }
      "file" => {
        let name = field.file_name().unwrap_or_default().to_owned();
        let mime = field.content_type().unwrap_or_default().to_owned();
        let bytes = field.bytes().await.map_err(unreadable)?;

        read.given.push(OwnerFile {
          name,
          mime,
          bytes: bytes.to_vec(),
        });
      }
      _ => {}
    }
  }

  Ok(read)
}

/// A form of files that can be put into a room: refused where it carries none, where the receiver does not hold the
/// room, and where the room is archived.
pub(super) async fn sendable(receiver: &Receiver, chat_id: i64, form: &FilesForm) -> Result<(), Refusal> {
  if form.given.is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "chatId (number) and file are required"));
  }

  if !known(receiver, chat_id).await? {
    return Err(refused(StatusCode::NOT_FOUND, "no-route", &format!("unknown chat {chat_id}")));
  }

  locked(receiver, chat_id).await
}

pub async fn say_file(State(served): State<Served>, writer: Writer) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let writer = writer.owners();
  let form = files_form(writer.form().await?).await?;
  let Some(chat_id) = form.chat_id else {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "chatId (number) and file are required"));
  };

  writer.room(&receiver, chat_id).await?;
  sendable(&receiver, chat_id, &form).await?;

  let speaking = Speaking::owner(&receiver, chat_id).await;

  files_said(&receiver, chat_id, form, speaking).await
}

/// The files of a form kept in a room under who speaks them, each as its own message with the caption beside it,
/// and carried to the room's other providers as one send of files; or one recording, where the form is a voice note.
pub(super) async fn files_said(
  receiver: &Receiver,
  chat_id: i64,
  form: FilesForm,
  speaking: Speaking,
) -> Answered {
  let FilesForm {
    caption,
    reply,
    voice,
    mut given,
    ..
  } = form;
  // The caption is read for secret-shaped runs before anything keeps or carries it (docs/messages.md, 「Secrets
  // taken out」).
  let caption = secrets::redacted(&caption);
  let reply_value = reply.map(Value::String);
  let reply = reply_target(receiver, chat_id, reply_value.as_ref()).await?;

  if voice {
    if given.len() != 1 {
      return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "a voice note carries one sound file"));
    }

    return say_voice(receiver, chat_id, reply, caption, given.remove(0), speaking).await;
  }

  let mut turned_away: Vec<Value> = vec![];
  let mut kept: Vec<OwnerFile> = vec![];

  for file in given {
    match refusal(&file.name, u64::try_from(file.bytes.len()).unwrap_or(0)) {
      Some(reason) => turned_away.push(json!({ "name": file.name, "reason": reason })),
      None => kept.push(file.clone()),
    }
  }

  if kept.is_empty() {
    let why = turned_away
      .first()
      .map(|held| held.get("reason").and_then(Value::as_str).unwrap_or_default().to_owned())
      .unwrap_or("no file stood".to_owned());

    return Err(refused(StatusCode::PAYLOAD_TOO_LARGE, "too-large", &why));
  }

  let mut sent: Vec<Value> = vec![];
  let mut first: Option<Inbound> = None;

  for (at, file) in kept.iter().enumerate() {
    let message_id = record::next_number(receiver.db(), chat_id).await.map_err(store)?;
    let message = Inbound {
      chat_id,
      message_id,
      sender_id: Some(speaking.sender_id),
      sender_name: speaking.name.clone(),
      sent_at: now_ms(),
      text: caption.clone(),
      image_path: None,
      image_state: None,
      transcript: None,
      voice_seconds: None,
      voice_state: None,
      reply_to_message_id: if at == 0 { reply } else { None },
      via: Road::Record.key().to_owned(),
      mode: None,
      via_reader: None,
      album: None,
    };
    // The record hands the file over as its own message's one file, identified by that place.
    let item = MessageFile {
      id: "1".to_owned(),
      name: file.name.clone(),
      mime: file.mime.clone(),
      bytes: file.bytes.clone(),
      kind: None,
    };
    let room = receiver.roads().room_facts(chat_id).await;

    Receiver::attachment(
      receiver,
      &message,
      &Context {
        carries_file: true,
        ..Context::default()
      },
      room,
      &[item],
    )
    .await
    .map_err(failed)?;
    sent.push(json!({ "name": file.name, "id": message_id }));
    first.get_or_insert(message);
  }

  // The files go on to the room's other providers as one send of files, the way they were put in together.
  if let Some(first) = first {
    let files = kept
      .iter()
      .map(|file| FileItem {
        name: file.name.clone(),
        mime: file.mime.clone(),
        bytes: file.bytes.clone(),
      })
      .collect();

    carried(receiver, &first, speaking.said_by, Carried::Files { files, caption }).await?;
  }

  answered(
    json!({ "id": sent.first().and_then(|held| held.get("id")), "sent": sent, "checking": [], "failed": turned_away }),
  )
}

// One recording the console made, kept in the room's record under the policy's voice word and the caption beside it,
// and carried to the room's other providers as a voice message (docs/api.md, 「Sends」).
async fn say_voice(
  receiver: &Receiver,
  chat_id: i64,
  reply: Option<i64>,
  caption: String,
  file: OwnerFile,
  speaking: Speaking,
) -> Answered {
  if let Some(reason) = refusal(&file.name, u64::try_from(file.bytes.len()).unwrap_or(0)) {
    return Err(refused(StatusCode::PAYLOAD_TOO_LARGE, "too-large", &reason));
  }

  let mime = file.mime.trim().to_ascii_lowercase();

  if !mime.starts_with("audio/") {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "a voice note carries one sound file"));
  }

  let message_id = record::next_number(receiver.db(), chat_id).await.map_err(store)?;
  let text = match caption.trim() {
    "" => ROADS.marks.voice.clone(),
    said => format!("{} {said}", ROADS.marks.voice),
  };
  let message = Inbound {
    chat_id,
    message_id,
    sender_id: Some(speaking.sender_id),
    sender_name: speaking.name,
    sent_at: now_ms(),
    text,
    image_path: None,
    image_state: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
    reply_to_message_id: reply,
    via: Road::Record.key().to_owned(),
    mode: None,
    via_reader: None,
    album: None,
  };
  let item = MessageFile {
    id: "1".to_owned(),
    name: file.name.clone(),
    mime: file.mime.clone(),
    bytes: file.bytes.clone(),
    kind: Some(ChatFileKind::VoiceNote),
  };
  let room = receiver.roads().room_facts(chat_id).await;

  Receiver::attachment(
    receiver,
    &message,
    &Context {
      carries_file: true,
      spoken_text: (!caption.trim().is_empty()).then(|| caption.clone()),
      ..Context::default()
    },
    room,
    &[item],
  )
  .await
  .map_err(failed)?;
  carried(
    receiver,
    &message,
    speaking.said_by,
    Carried::Voice {
      audio: file.bytes,
      caption,
    },
  )
  .await?;

  answered(json!({
    "id": message_id,
    "sent": [{ "name": file.name, "id": message_id }],
    "checking": [],
    "failed": [],
  }))
}

#[derive(Deserialize)]
pub struct ForwardBody {
  to: i64,
  #[serde(default)]
  name: Option<String>,
}

// Who a forward is kept as: the owner where it names no account, and otherwise who the account speaks as, under the
// shown name the call gives, else the name its identity last went by, else its key.
async fn forwarder(
  receiver: &Receiver,
  room: i64,
  account: Option<i64>,
  name: Option<&str>,
) -> Result<Speaking, Refusal> {
  let Some(account) = account else {
    return Ok(Speaking::owner(receiver, room).await);
  };
  let given = name.map(str::trim).filter(|name| !name.is_empty());
  let speaking = speaker(receiver, room, account, given.unwrap_or_default()).await?;

  if given.is_some() || speaking.said_by == SaidBy::Owner {
    return Ok(speaking);
  }

  let name = reads::display_name(receiver.db(), speaking.sender_id)
    .await
    .map_err(store)?
    .unwrap_or_else(|| account.to_string());

  Ok(Speaking {
    sender_id: speaking.sender_id,
    name: name.clone(),
    said_by: SaidBy::Account { account, name },
  })
}

/// A message forwarded into another room (docs/api.md, 「Sends」): a copy kept there as the forwarding account's, the
/// owner's where the call names none, marked with where it came from and who said it, carried out on that room's own
/// road, waking nobody. Nothing is forwarded out of or into another account's private room, since a forward shares
/// what was said in the one and writes in the other.
pub async fn forward(
  State(served): State<Served>,
  Path((_, message)): Path<(i64, i64)>,
  writable: WritableRoom,
) -> Result<(StatusCode, Json<Value>), Refusal> {
  let room = writable.room;
  let writer = writable.writer;
  let asked = read::<ForwardBody>(writer.body())?;
  let receiver: Receiver = (*served.receiver).clone();

  if asked.to == room {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      "a message is forwarded into another room",
    ));
  }

  let Some(source) =
    crate::receiver::reads::message_of(receiver.db(), room, message).await.map_err(store)?
  else {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The receiver keeps no message {message} in room {room}."),
    ));
  };

  if source.deleted_at.is_some() {
    return Err(refused(StatusCode::CONFLICT, "conflict", &format!("Message {message} was deleted.")));
  }

  if !known(&receiver, asked.to).await? {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The receiver holds no room {}.", asked.to),
    ));
  }

  writer.room(&receiver, asked.to).await?;

  let account = writer.account();

  let Speaking {
    sender_id,
    name,
    said_by,
  } = forwarder(&receiver, asked.to, account, asked.name.as_deref()).await?;
  let kept = record::Kept {
    chat_id: asked.to,
    number: None,
    text: &source.text,
    reply_to: None,
    via: Road::Record.key(),
    sender_id: Some(sender_id),
    name: &name,
    agent: None,
    voice: None,
  };
  let copy = record::keep(receiver.db(), receiver.news(), kept).await.map_err(store)?;
  // The picture the message stores is kept again under the copy, and goes on to the room's other providers
  // with it; a file no longer there leaves the copy with the words alone.
  let picture = source.image_path.as_deref().and_then(|stored| stored_picture(&receiver, room, stored));

  if let Some(file) = &picture {
    let inbound = Inbound {
      chat_id: asked.to,
      message_id: copy,
      sender_id: Some(sender_id),
      sender_name: name.clone(),
      sent_at: now_ms(),
      text: source.text.clone(),
      image_path: None,
      image_state: None,
      transcript: None,
      voice_seconds: None,
      voice_state: None,
      reply_to_message_id: None,
      via: Road::Record.key().to_owned(),
      mode: None,
      via_reader: None,
      album: None,
    };

    crate::receiver::files::keep(
      receiver.db(),
      receiver.data_root(),
      receiver.deployment().attachment_bytes,
      &inbound,
      std::slice::from_ref(file),
    )
    .await
    .map_err(store)?;
  }

  crate::run(
    receiver.db(),
    r#"UPDATE "messages" SET "forwarded_room" = $3, "forwarded_message" = $4, "forwarded_name" = $5
       WHERE "chat_id" = $1 AND "message_id" = $2"#,
    vec![
      asked.to.into(),
      copy.into(),
      room.into(),
      message.into(),
      source.sender_name.clone().into(),
    ],
  )
  .await
  .map_err(store)?;

  let delivery = Delivery {
    chat_id: asked.to,
    message_id: copy,
    reply_to: None,
    said_by,
    standing: vec![Road::Record.key().to_owned()],
    carried: match picture {
      Some(file) => Carried::Files {
        files: vec![FileItem {
          name: file.name,
          mime: file.mime,
          bytes: file.bytes,
        }],
        caption: source.text.clone(),
      },
      None => Carried::words(&source.text),
    },
  };

  providers::deliver(receiver.db(), receiver.roads(), &delivery).await;

  Ok((StatusCode::CREATED, Json(json!({ "room": asked.to, "id": copy }))))
}

// The picture a message of a room stores, read back as the one file of a message: none where the stored path
// does not stand inside the room's picture folder or the file is no longer there.
fn stored_picture(receiver: &Receiver, room: i64, stored: &str) -> Option<MessageFile> {
  let folder = room_path(&receiver.data_root().images(), room);
  let real = inside(&folder, stored)?;
  let bytes = std::fs::read(&real).ok()?;
  let name = real.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
  let mime = mime_guess::from_path(&real).first_or_octet_stream().essence_str().to_owned();

  Some(MessageFile {
    id: "1".to_owned(),
    name,
    mime,
    bytes,
    kind: None,
  })
}

/// A refusal where the room is archived: nothing more is said in it (docs/receiver.md, 「An archived room」).
pub(super) async fn locked(receiver: &Receiver, chat_id: i64) -> Result<(), super::Refusal> {
  match crate::roads::archived::archived(receiver.db(), chat_id).await.map_err(store)? {
    Some(_) => {
      let refusal = receiver_codes::OwnerSendRefusal::RoomArchived { room: chat_id };
      Err(refused(StatusCode::CONFLICT, (&refusal).into(), &refusal.to_string()))
    }
    None => Ok(()),
  }
}

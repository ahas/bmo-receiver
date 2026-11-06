//! The nouns this repository's unit command declares to `bmo` (bmo-cli's `docs/api.md`, 「The unit command」;
//! docs/boundary-api.md, 「The unit command」): the sends a shell asks the running receiver for through
//! `POST /sends`, a line in the consult room through `POST /consult-lines`, the consult room read or set, the
//! fair draw made or checked, and an agent's standing session asked for its handoff file through
//! `POST /handoffs/{agentName}`. Every noun runs on the operations machine, since the receiver, its store and the
//! rooms it answers are that machine's.

use std::io::Read;

use clap::{Args, Subcommand};
use receiver_client::{HandoffAsked, ReceiverClient, RoomMode, RoomRegistration};
use room_calls::bot::Service;
use room_calls::sends::{
  Act, ConsultInput, PhotoInput, Picture, PinInput, RandomDrawInput, ReactInput, SayInput, StickerInput,
};
use serde_json::{Map, Value};

use crate::config::Config;
use crate::receiver::Receiver;
use crate::tool_env::base_address;
use crate::verbs::draws::{self, Claim, Claimed};
use crate::verbs::{VerbError, asked_sends, consult, messages, turn};

/// The nouns `bmo` dispatches to this command, each marked `home`: the receiver's rooms, its consult room, the
/// draws its store keeps and the sessions it hands over are the operations machine's.
pub const NOUNS: [&str; 10] = [
  "room say\thome",
  "room registration\thome",
  "talk\thome",
  "photo\thome",
  "react\thome",
  "pin\thome",
  "sticker\thome",
  "consult\thome",
  "draw\thome",
  "handoff\thome",
];

/// The words of `room say`: the room, the words and who set them off.
#[derive(Args, Debug)]
pub struct SayArgs {
  /// The room; the owner's own chat where none is named.
  #[arg(long)]
  pub chat: Option<i64>,
  /// Who set this off: chat:<chatId> for a chat session, told there what became of the line.
  #[arg(long)]
  pub by: Option<String>,
  /// Asks the room for an answer back, which comes to the chat `--by` names.
  #[arg(long)]
  pub reply: bool,
  /// The message this line replies to, by its number in the room.
  #[arg(long = "reply-to")]
  pub reply_to: Option<i64>,
  /// The words; read from standard input where none are given.
  #[arg(long)]
  pub text: Option<String>,
}

/// The words of `room registration`: one room's registration as the administrator named, and its mode changed
/// where one is named.
#[derive(Args, Debug)]
pub struct RegistrationArgs {
  /// The room.
  #[arg(long)]
  pub room: i64,
  /// The mode: `record-only` or `avatar`; the registration is only read where none is named.
  #[arg(long, value_parser = ["record-only", "avatar"])]
  pub mode: Option<String>,
  /// The administrator's name, as the web unit knows it.
  #[arg(long)]
  pub by: String,
}

/// The words of `talk`: a line said in a room, or spoken in a voice and sent as a voice note with the words
/// as what it says. The mouth group's first-class verb.
#[derive(Args, Debug)]
pub struct TalkArgs {
  #[command(flatten)]
  pub say: SayArgs,
  /// The trained voice the line is spoken in; the line goes as text where none is named.
  #[arg(long)]
  pub voice: Option<String>,
  /// The spoken words the voice synthesises, the line as a Korean reader says it aloud; the voice engine
  /// derives them from the words where none are given. The words stay what the note says either way.
  #[arg(long, requires = "voice")]
  pub spoken: Option<String>,
}

/// The words of `photo`: one picture or an album, each with the word drawn into it.
#[derive(Args, Debug)]
pub struct PhotoArgs {
  /// The room; the owner's own chat where none is named.
  #[arg(long)]
  pub chat: Option<i64>,
  /// A web address or a file this machine keeps, once for each picture.
  #[arg(long = "url", required = true)]
  pub urls: Vec<String>,
  /// The word drawn into the picture of the same place among the `--url`s.
  #[arg(long = "label")]
  pub labels: Vec<String>,
  /// The caption.
  #[arg(long)]
  pub caption: Option<String>,
}

/// The words of `react`: an emoji put on a message, or `none` to clear it.
#[derive(Args, Debug)]
pub struct ReactArgs {
  #[arg(long)]
  pub chat: i64,
  #[arg(long)]
  pub message: i64,
  /// The emoji, or `none` to clear the agent's own.
  #[arg(long)]
  pub emoji: String,
}

/// The words of `pin`: a message pinned, or unpinned with `--unpin`.
#[derive(Args, Debug)]
pub struct PinArgs {
  #[arg(long)]
  pub chat: i64,
  #[arg(long)]
  pub message: i64,
  #[arg(long)]
  pub unpin: bool,
}

/// The words of `sticker`: one of the deployment's stickers, or the set listed with `--list`.
#[derive(Args, Debug)]
pub struct StickerArgs {
  #[arg(long, required_unless_present = "list")]
  pub chat: Option<i64>,
  #[arg(long, required_unless_present = "list")]
  pub emoji: Option<String>,
  #[arg(long, conflicts_with_all = ["chat", "emoji"])]
  pub list: bool,
}

/// The words of `consult`: the room read or set, the block an agent is shown, or a line said.
#[derive(Args, Debug)]
pub struct ConsultArgs {
  #[command(subcommand)]
  pub word: ConsultWord,
  /// The agent the line or the block is; the one the calling run names where none is.
  #[arg(long, global = true)]
  pub agent: Option<String>,
}

/// The verbs of `consult`.
#[derive(Subcommand, Debug)]
pub enum ConsultWord {
  /// The consult room as it is set, or with a room named set anew to it.
  Room {
    /// The room, by number or title.
    room: Option<String>,
  },
  /// The block an agent is shown of the other agents' work, as it would read it now.
  State,
  /// One line said in the consult room under the agent's own name.
  Say {
    /// The words.
    text: String,
    /// The consult room's message these words answer, by its number.
    #[arg(long)]
    reply: Option<i64>,
  },
}

/// The words of `draw draw`: a draw made in a room, as the tool makes it.
#[derive(Args, Debug)]
pub struct DrawArgs {
  #[arg(long)]
  pub chat: Option<String>,
  /// The request, as the tool's own fields in one JSON object.
  #[arg(long)]
  pub spec: Option<String>,
}

/// The words of `draw verify`: a draw checked by its number, or one checked from its room line alone.
#[derive(Args, Debug)]
pub struct VerifyArgs {
  /// The draw's number, in the store.
  pub draw_id: Option<String>,
  #[arg(long)]
  pub seed: Option<String>,
  #[arg(long)]
  pub draw: Option<String>,
  #[arg(long)]
  pub spec: Option<String>,
  #[arg(long)]
  pub commitment: Option<String>,
  #[arg(long)]
  pub result: Option<String>,
}

/// The verbs of `draw`.
#[derive(Subcommand, Debug)]
pub enum DrawWord {
  /// A draw made in a room, as the tool makes it.
  #[command(allow_negative_numbers = true)]
  Draw(DrawArgs),
  /// A draw checked: out of the store by its number, or out of what its room line published alone.
  #[command(allow_negative_numbers = true)]
  Verify(VerifyArgs),
}

/// The words of `handoff`: the agent whose standing session is asked to write its handoff file ahead of a fresh
/// one, why, and the room it keeps that session in where it keeps one in each room.
#[derive(Args, Debug)]
pub struct HandoffArgs {
  /// The agent whose standing session is asked.
  pub agent: String,
  /// Why a fresh session is coming, in a short English sentence.
  #[arg(long)]
  pub reason: String,
  /// The room whose session is asked, where the agent keeps one in each room.
  #[arg(long)]
  pub room: Option<i64>,
}

/// The words given, or those on standard input where none are.
pub(crate) fn words(text: Option<String>) -> Result<String, VerbError> {
  match text {
    Some(text) => Ok(text),
    None => {
      let mut read = String::new();

      std::io::stdin().read_to_string(&mut read).map_err(|why| {
        VerbError::Refused(format!("the words could not be read from standard input: {why}"))
      })?;

      Ok(read)
    }
  }
}

/// `room say`: a line said in a room through `POST /sends`, by a run of its own the receiver starts.
pub async fn room_say(
  config: &Config,
  via: Option<String>,
  args: SayArgs,
) -> Result<Vec<String>, VerbError> {
  let chat = args.chat.unwrap_or(config.owner);
  let asker = asked_sends::shell_asker(chat, via, args.by, "room say");
  let input = SayInput {
    chat: None,
    via: None,
    text: words(args.text)?,
    reply: args.reply,
    reply_to: args.reply_to,
  };

  asked_sends::ask(&config.data_root, asker, Act::Say(input)).await
}

// The running receiver the registration verbs reach, at the address its configuration names.
fn client(config: &Config) -> Result<ReceiverClient, VerbError> {
  let http = reqwest::Client::builder()
    .no_proxy()
    .build()
    .map_err(|why| VerbError::Refused(format!("no client for the receiver: {why}")))?;
  let client =
    ReceiverClient::new(&base_address(&config.listen), config.token.clone()).map_err(VerbError::Refused)?;

  Ok(client.with_http(http))
}

// A room's registration as the verb prints it: its mode, its providers, whether it is read-only and why, and its
// answerers.
fn registration_lines(registration: &RoomRegistration) -> Vec<String> {
  let mode = match registration.mode {
    Some(RoomMode::Avatar) => "avatar",
    Some(RoomMode::RecordOnly) => "record-only",
    None => "none",
  };
  let providers = if registration.providers.is_empty() {
    "none".to_owned()
  } else {
    registration.providers.join(", ")
  };
  let mut lines = vec![
    format!("room {}", registration.room),
    format!("mode {mode}"),
    format!(
      "providers {providers} ({})",
      if registration.chosen { "chosen" } else { "read off the room" }
    ),
  ];

  if registration.read_only {
    lines.push(format!(
      "read-only yes, read in the owner's account alone by {}",
      registration.read_only_by.join(", ")
    ));
  } else if registration.read_only_by.is_empty() {
    lines.push("read-only no".to_owned());
  } else {
    lines.push(format!(
      "read-only no, though {} reads it in the owner's account alone",
      registration.read_only_by.join(", ")
    ));
  }

  let named: Vec<&str> = registration
    .answerers
    .items
    .iter()
    .map(|answerer| answerer.agent_name.as_str())
    .collect();

  lines.push(format!(
    "answerers default {}, voice {}, named {}",
    registration.answerers.default,
    registration.answerers.voice,
    if named.is_empty() { "none".to_owned() } else { named.join(", ") }
  ));

  lines
}

/// `room registration`: a room's registration read through `GET /rooms/{roomId}/registration`, or its mode
/// changed through `PUT` on the same route where one is named.
pub async fn room_registration(config: &Config, args: RegistrationArgs) -> Result<Vec<String>, VerbError> {
  let client = client(config)?;
  let registration = match args.mode.as_deref() {
    None => client.room_registration(args.room, args.by.trim()).await,
    Some("avatar") => client.change_room_mode(args.room, args.by.trim(), RoomMode::Avatar).await,
    Some("record-only") => client.change_room_mode(args.room, args.by.trim(), RoomMode::RecordOnly).await,
    Some(other) => {
      return Err(VerbError::Refused(format!("--mode names {other}, not record-only or avatar")));
    }
  }
  .map_err(|why| VerbError::Refused(why.to_string()))?;

  Ok(registration_lines(&registration))
}

/// `talk`: a line into a room as text, or spoken in a voice and sent as a voice note.
pub async fn talk(config: &Config, via: Option<String>, args: TalkArgs) -> Result<Vec<String>, VerbError> {
  crate::verbs::talk::talk(config, via, args.say, args.voice, args.spoken).await
}

/// `photo`: one picture or an album through `POST /sends`.
pub async fn photo(
  config: &Config,
  via: Option<String>,
  args: PhotoArgs,
) -> Result<Vec<String>, VerbError> {
  let chat = args.chat.unwrap_or(config.owner);
  let pictures: Vec<Picture> = args
    .urls
    .into_iter()
    .enumerate()
    .map(|(at, url)| Picture {
      url,
      label: args.labels.get(at).cloned(),
    })
    .collect();
  let input = PhotoInput {
    chat: None,
    via: None,
    pictures,
    caption: args.caption,
    reply_to: None,
  };

  asked_sends::ask(&config.data_root, asked_sends::shell_asker(chat, via, None, "photo"), Act::Photo(input))
    .await
}

/// `react`: an emoji put on a message, or cleared, through `POST /sends`.
pub async fn react(
  config: &Config,
  via: Option<String>,
  args: ReactArgs,
) -> Result<Vec<String>, VerbError> {
  let input = ReactInput {
    chat: None,
    via: None,
    message: args.message,
    emoji: args.emoji,
  };

  asked_sends::ask(
    &config.data_root,
    asked_sends::shell_asker(args.chat, via, None, "react"),
    Act::React(input),
  )
  .await
}

/// `pin`: a message pinned, or unpinned, through `POST /sends`.
pub async fn pin(config: &Config, via: Option<String>, args: PinArgs) -> Result<Vec<String>, VerbError> {
  let input = PinInput {
    chat: None,
    via: None,
    message: args.message,
    unpin: args.unpin,
  };

  asked_sends::ask(
    &config.data_root,
    asked_sends::shell_asker(args.chat, via, None, "pin"),
    Act::Pin(input),
  )
  .await
}

/// `sticker`: one of the deployment's stickers through `POST /sends`, or the set listed.
pub async fn sticker(
  config: &Config,
  via: Option<String>,
  args: StickerArgs,
) -> Result<Vec<String>, VerbError> {
  let (Some(chat), Some(emoji)) = (args.chat, args.emoji) else {
    return messages::sticker_list();
  };
  let input = StickerInput {
    chat: None,
    via: None,
    emoji,
    reply_to: None,
  };

  asked_sends::ask(
    &config.data_root,
    asked_sends::shell_asker(chat, via, None, "sticker"),
    Act::Sticker(input),
  )
  .await
}

/// `consult`: the consult room read or set and the block an agent is shown from the store, and a line said
/// through `POST /consult-lines`.
pub async fn consult(config: &Config, args: ConsultArgs) -> Result<Vec<String>, VerbError> {
  let wanted = args.agent.or_else(turn::agent);

  match args.word {
    ConsultWord::Room { room } => {
      let receiver = store(config).await?;

      consult::room(&receiver, room.as_deref()).await.map_err(VerbError::Refused)
    }
    ConsultWord::State => {
      let Some(agent) = wanted else {
        return Err(VerbError::Refused(
          "consult state: no agent is named: run this inside an agent's turn, or name one with --agent <name>"
            .to_owned(),
        ));
      };
      let receiver = store(config).await?;

      Ok(consult::state(&receiver, &agent).await)
    }
    ConsultWord::Say { text, reply } => {
      let Some(agent) = wanted else {
        return Err(VerbError::Refused(
          "consult say: no agent is named: run this inside an agent's turn, or name one with --agent <name>"
            .to_owned(),
        ));
      };
      let input = ConsultInput { text, reply };
      let asker = asked_sends::shell_turn("consult say");

      asked_sends::said(
        room_calls::sends::ask_consult(&Service::of(&config.data_root), Some(&agent), &asker, &input).await,
      )
    }
  }
}

/// `draw`: a draw made in a room through `POST /sends`, or one checked against the method and the store.
pub async fn draw(config: &Config, via: Option<String>, word: DrawWord) -> Result<Vec<String>, VerbError> {
  match word {
    DrawWord::Draw(args) => draw_made(config, via, args).await,
    DrawWord::Verify(args) => draw_checked(config, args).await,
  }
}

/// `handoff`: the agent's standing session asked for its handoff file through `POST /handoffs/{agentName}`, the
/// answer the route gives printed: the file it wrote and its size, or that it wrote nothing. Nothing of a room's
/// conversation, its logs or its runs is changed; the route writes the note and the fresh session opens on it.
pub async fn handoff(config: &Config, args: HandoffArgs) -> Result<Vec<String>, VerbError> {
  let agent = args.agent.trim();
  let reason = args.reason.trim();

  if agent.is_empty() {
    return Err(VerbError::Refused("handoff: an agent name is required".to_owned()));
  }

  let asked = HandoffAsked {
    reason: reason.to_owned(),
    room: args.room,
  };
  let written = client(config)?
    .hand_off(agent, &asked)
    .await
    .map_err(|why| VerbError::Refused(why.to_string()))?;
  let asked_line = format!("handoff {agent}: asked ({reason})");

  Ok(match written {
    Some(written) => {
      vec![asked_line, format!("handoff {agent}: wrote {} ({} bytes)", written.path, written.bytes)]
    }
    None => vec![
      asked_line,
      format!("handoff {agent}: wrote nothing: no standing session was asked, or it wrote no file"),
    ],
  })
}

/// `draw draw`: the request read and the send asked of `POST /sends`.
async fn draw_made(config: &Config, via: Option<String>, args: DrawArgs) -> Result<Vec<String>, VerbError> {
  let chat =
    draws::whole(args.chat.as_deref()).ok_or_else(|| VerbError::Refused(draws::USAGE.to_owned()))?;
  let fields: Map<String, Value> = draws::spec_argument(args.spec.as_deref())?;
  let input: RandomDrawInput = serde_json::from_value(Value::Object(fields))
    .map_err(|why| VerbError::Refused(format!("--spec does not name a draw: {why}")))?;
  let asker = asked_sends::shell_asker(chat, via, None, "draw draw");

  asked_sends::ask(&config.data_root, asker, Act::RandomDraw(input)).await
}

/// `draw verify`: one recorded draw checked by its number, or a published claim checked from its room line.
async fn draw_checked(config: &Config, args: VerifyArgs) -> Result<Vec<String>, VerbError> {
  if args.seed.is_none() {
    let draw_id =
      draws::whole(args.draw_id.as_deref()).ok_or_else(|| VerbError::Refused(draws::USAGE.to_owned()))?;
    let db = config.connect().await.map_err(VerbError::Refused)?;

    return draws::judged(draws::verify_recorded(&db, draw_id).await?);
  }

  let draw_id =
    draws::whole(args.draw.as_deref()).ok_or_else(|| VerbError::Refused(draws::USAGE.to_owned()))?;
  let claim = Claim {
    seed: args.seed.unwrap_or_default(),
    draw_id,
    spec: draws::spec_argument(args.spec.as_deref())?,
    method: None,
    commitment: args.commitment,
    result: args.result.map(Claimed::Text),
  };

  draws::judged(draws::verify_published(&claim))
}

/// The receiver's store read as a short-lived command reads it: the configuration's database connected, and
/// the receiver standing on it for the verbs that read a room's own record.
async fn store(config: &Config) -> Result<Receiver, VerbError> {
  let db = config.connect().await.map_err(VerbError::Refused)?;

  Ok(config.receiver(db, Vec::new()))
}

//! The sends the agents make into rooms, the entries of the `send` tool (docs/chat-mcp.md, 「Its entries」;
//! 「How a call runs」): a line said in a room or in the consult room, a picture or an album, a file or an album
//! of files, a voice note, a reaction, a pin, a sticker, a fair draw and a fixed line. Every entry asks the
//! running receiver for the send, as the `bmo` commands doing the same sends ask it. A call reads its turn
//! afresh from the session's record and refuses one its agent's reset overtook before it asks anything, and
//! reaches each unit where the record's data root names it (docs/chat-mcp.md, 「Where each unit is found」).

use serde_json::{Value, json};

use receiver_ports::agency::HttpAgency;
use room_calls::{
  bot::Service,
  sends::{
    Act, Asker, ConsultInput, FileInput, FixedLineInput, PhotoInput, PinInput, RandomDrawInput, ReactInput,
    SayInput, StickerInput, Unsent, VoiceInput, ask, ask_consult,
  },
  turn::{NO_ROOM, Turn},
};
use tool_call::{Answer, Asked, Entry, Handed, MadeSchema, Session, input_schema, peers};

/// The catalogue's capability every entry is shown by.
const CATALOGUE: &[&str] = &["room.tools"];

// A send reaches past this machine into a room, takes nothing away, and a second call sends a second time.
const REACHES_OUT: &[(&str, bool)] = &[("open_world", true)];

const fn entry(
  name: &'static str,
  description: &'static str,
  input: &'static str,
  schema: MadeSchema,
  grant: Option<&'static str>,
) -> Entry {
  Entry {
    namespace: "bmo",
    name,
    title: None,
    description,
    input,
    input_schema: schema,
    output: None,
    capabilities: CATALOGUE,
    hints: REACHES_OUT,
    grant,
  }
}

/// Every entry the executable answers, as its manifest declares them.
pub const ENTRIES: &[Entry] = &[
  entry(
    "say",
    "Put a line in a room, through the output gates, without waiting for it.",
    "say.json",
    input_schema::<SayInput>,
    None,
  ),
  entry(
    "consult",
    "Say a line in the consult room under your own name, to settle work of yours that overlaps another agent's; answers with each agent's window there.",
    "consult.json",
    input_schema::<ConsultInput>,
    None,
  ),
  entry(
    "photo",
    "Send a picture, or several as one album.",
    "photo.json",
    input_schema::<PhotoInput>,
    None,
  ),
  entry(
    "file",
    "Send a file from this machine, or several as one album, with a caption.",
    "file.json",
    input_schema::<FileInput>,
    None,
  ),
  entry(
    "voice",
    "Send an OGG/Opus voice note from this machine, with its caption.",
    "voice.json",
    input_schema::<VoiceInput>,
    None,
  ),
  entry(
    "react",
    "Put an emoji reaction on a message, or clear the agent's own with an empty emoji.",
    "react.json",
    input_schema::<ReactInput>,
    None,
  ),
  entry("pin", "Pin a message quietly, or unpin it.", "pin.json", input_schema::<PinInput>, None),
  entry(
    "sticker",
    "Send one of the deployment's stickers by its emoji.",
    "sticker.json",
    input_schema::<StickerInput>,
    None,
  ),
  entry(
    "random_draw",
    "Draw fairly for anything decided by chance — dice, a number, winners, an order, a tie-break — and post the result with its seed in the room. sync hands the result back to state in this reply; async hands it to the next turn, for a turn that announces the draw and ends.",
    "random_draw.json",
    input_schema::<RandomDrawInput>,
    None,
  ),
  entry(
    "fixed_line",
    "Say a fixed line in a room by its code and fields, rendered by the web unit's language packs without the output gates or any session.",
    "fixed_line.json",
    input_schema::<FixedLineInput>,
    None,
  ),
];

// What the receiver said of a send: its lines, or those it said before the refusal with the refusal last.
fn said(answered: Result<Vec<String>, Unsent>) -> Answer {
  match answered {
    Ok(lines) => Answer::text(lines.join("\n")),
    Err(Unsent::Unreached(why)) => Answer::failure(why),
    Err(unsent) => Answer::refusal(unsent.lines().join("\n")),
  }
}

/// One send asked of the running receiver as the call's turn, in the room the call names or the turn's own, on
/// the road the turn names for that room alone.
async fn send(
  receiver: &Service,
  agency: &HttpAgency,
  turn: &Turn,
  chat: Option<i64>,
  via: Option<String>,
  act: Act,
) -> Answer {
  if let Some(refused) = turn.overtaken(agency).await {
    return Answer::refusal(refused);
  }
  let Some(room) = turn.room(chat) else { return Answer::refusal(NO_ROOM) };
  let mut asker = Asker::of_turn(turn, Some(&room));

  if via.is_some() {
    asker.via = via;
  }

  said(ask(receiver, &room_calls::sends::Asked { asker, act }).await)
}

/// A line said in the consult room under the calling agent's own name; a call outside any agent's turn has no
/// agent to speak as and is refused before the receiver is asked.
async fn consulted(receiver: &Service, agency: &HttpAgency, turn: &Turn, input: ConsultInput) -> Answer {
  if let Some(refused) = turn.overtaken(agency).await {
    return Answer::refusal(refused);
  }

  said(ask_consult(receiver, turn.agent.as_deref(), &input).await)
}

fn read<T: serde::de::DeserializeOwned>(name: &str, arguments: Value) -> Result<T, Answer> {
  serde_json::from_value(arguments)
    .map_err(|why| Answer::failure(format!("invalid arguments: {name}: {why}")))
}

/// One call of an entry, by its name, with its arguments, answered through the units the record's data root
/// reaches.
pub async fn call(session: &Session, name: &str, arguments: Value) -> Answer {
  let receiver = Service::of(&session.data_root);
  let agency = HttpAgency::new(&peers::agency(&session.data_root));
  let turn = Turn::of(&session.turn_variables());

  let asked = async {
    Ok::<Answer, Answer>(match name {
      "say" => {
        let input: SayInput = read(name, arguments)?;

        send(&receiver, &agency, &turn, input.chat, input.via.clone(), Act::Say(input)).await
      }
      "consult" => {
        let input: ConsultInput = read(name, arguments)?;

        consulted(&receiver, &agency, &turn, input).await
      }
      "photo" => {
        let input: PhotoInput = read(name, arguments)?;

        send(&receiver, &agency, &turn, input.chat, input.via.clone(), Act::Photo(input)).await
      }
      "file" => {
        let input: FileInput = read(name, arguments)?;

        send(&receiver, &agency, &turn, input.chat, input.via.clone(), Act::File(input)).await
      }
      "voice" => {
        let input: VoiceInput = read(name, arguments)?;

        send(&receiver, &agency, &turn, input.chat, input.via.clone(), Act::Voice(input)).await
      }
      "react" => {
        let input: ReactInput = read(name, arguments)?;

        send(&receiver, &agency, &turn, input.chat, input.via.clone(), Act::React(input)).await
      }
      "pin" => {
        let input: PinInput = read(name, arguments)?;

        send(&receiver, &agency, &turn, input.chat, input.via.clone(), Act::Pin(input)).await
      }
      "sticker" => {
        let input: StickerInput = read(name, arguments)?;

        send(&receiver, &agency, &turn, input.chat, input.via.clone(), Act::Sticker(input)).await
      }
      "random_draw" => {
        let input: RandomDrawInput = read(name, arguments)?;

        send(&receiver, &agency, &turn, input.chat, input.via.clone(), Act::RandomDraw(input)).await
      }
      "fixed_line" => {
        let input: FixedLineInput = read(name, arguments)?;

        send(&receiver, &agency, &turn, input.chat, input.via.clone(), Act::FixedLine(input)).await
      }
      other => Answer::failure(format!("this executable holds no entry {other}")),
    })
  };

  asked.await.unwrap_or_else(|refused| refused)
}

/// One request answered: a call of an entry by its full name, resource requests answered as a tool that serves
/// none, and any other kind refused.
pub async fn answer(handed: Handed) -> Result<Value, String> {
  let answered = match handed.asked {
    Asked::Call {
      entry: asked,
      arguments,
    } => match strip_namespace(&asked) {
      Some(name) if ENTRIES.iter().any(|entry| entry.name == name) => {
        call(&handed.session, name, arguments).await
      }
      _ => Answer::failure(format!("this executable holds no entry {asked}")),
    },
    Asked::ResourcesList => return Ok(json!({ "resources": [] })),
    Asked::ResourcesRead { .. } => return Ok(json!({ "error": "this executable serves no resources" })),
    Asked::Other => return Err("this executable answers calls alone".to_owned()),
  };

  serde_json::to_value(answered).map_err(|why| format!("the answer could not be written: {why}"))
}

// The entry's name as the executable holds it, out of the call's full name.
fn strip_namespace(full: &str) -> Option<&str> {
  full.strip_prefix("bmo/")
}

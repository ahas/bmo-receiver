//! The rooms an agent opens, reads and names its answerers for (docs/chat-mcp.md): an agent room on the
//! record alone, its supervisors, and the rooms a supervisor answers. Every call asks the running receiver
//! for the room, as the `bmo` commands doing the same asks ask it.

use room_calls::bot::Service;
use room_calls::turn::Turn;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tool_call::{Answer, Asked, Entry, Handed, MadeSchema, Session, input_schema};

const CATALOGUE: &[&str] = &["room.tools"];
const REACHES_OUT: &[(&str, bool)] = &[("open_world", true)];

const fn entry(
  name: &'static str,
  description: &'static str,
  input: &'static str,
  schema: MadeSchema,
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
    grant: None,
  }
}

/// Every entry the executable answers, as its manifest declares them.
pub const ENTRIES: &[Entry] = &[
  entry(
    "room_open",
    "Open an agent room on the record alone, with the agents that answer it; the owner reads it as any room.",
    "open.json",
    input_schema::<OpenInput>,
  ),
  entry(
    "room_list",
    "The rooms a supervisor answers, each with its number and its title.",
    "list.json",
    input_schema::<ListInput>,
  ),
  entry(
    "room_supervisors",
    "Name the whole list of supervisors that answer a room.",
    "supervisors.json",
    input_schema::<SupervisorsInput>,
  ),
];

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenInput {
  /// The room's title, as a person reads it.
  title: String,
  /// The supervisors that answer it; the calling agent is added where it names none.
  supervisors: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListInput {
  /// The supervisor whose rooms are read; the calling agent where none is given.
  supervisor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SupervisorsInput {
  /// The room, by its number.
  room_id: i64,
  /// The whole list of supervisors that answer it.
  supervisors: Vec<String>,
}

fn read<T: serde::de::DeserializeOwned>(name: &str, arguments: Value) -> Result<T, Answer> {
  serde_json::from_value(arguments)
    .map_err(|why| Answer::failure(format!("the arguments of {name} do not read: {why}")))
}

struct Receiver {
  base: String,
  token: String,
}

impl Receiver {
  fn of(session: &Session) -> Result<Self, Answer> {
    let service = Service::of(&session.data_root);
    let token = service
      .token
      .filter(|token| !token.trim().is_empty())
      .ok_or_else(|| Answer::failure("the receiver's token stands in no setting this call reads"))?;

    Ok(Self {
      base: service.address.trim().to_owned(),
      token,
    })
  }

  fn client(&self) -> reqwest::Client {
    reqwest::Client::new()
  }

  async fn answered(&self, request: reqwest::RequestBuilder, path: &str) -> Answer {
    let answer = match request.bearer_auth(&self.token).send().await {
      Ok(answer) => answer,
      Err(why) => return Answer::failure(format!("the receiver did not answer {path}: {why}")),
    };
    let status = answer.status();
    let written: Value = answer.json().await.unwrap_or(Value::Null);

    if status.is_success() {
      return Answer::text(serde_json::to_string_pretty(&written).unwrap_or_default());
    }

    let why = written.get("error").and_then(Value::as_str).unwrap_or("no reason given");

    Answer::failure(format!("the receiver refused {path} ({}): {why}", status.as_u16()))
  }
}

/// One call of an entry, answered.
pub async fn call(session: &Session, name: &str, arguments: Value) -> Answer {
  let receiver = match Receiver::of(session) {
    Ok(receiver) => receiver,
    Err(answer) => return answer,
  };
  let turn = Turn::of(&session.turn_variables());
  let client = receiver.client();

  match name {
    "room_open" => {
      let input: OpenInput = match read(name, arguments) {
        Ok(input) => input,
        Err(answer) => return answer,
      };
      let mut members = input.supervisors;

      if let Some(agent) = turn.agent
        && !members.iter().any(|name| name == &agent)
      {
        members.push(agent);
      }

      let body = json!({ "title": input.title, "supervisors": members });
      receiver
        .answered(client.post(format!("{}/rooms", receiver.base)).json(&body), "/rooms")
        .await
    }
    "room_list" => {
      let input: ListInput = match read(name, arguments) {
        Ok(input) => input,
        Err(answer) => return answer,
      };
      let Some(supervisor) = input.supervisor.or(turn.agent) else {
        return Answer::failure("the call names no supervisor and stands in no agent's turn");
      };
      let path = format!("/supervisors/{supervisor}/rooms");

      receiver.answered(client.get(format!("{}{path}", receiver.base)), &path).await
    }
    "room_supervisors" => {
      let input: SupervisorsInput = match read(name, arguments) {
        Ok(input) => input,
        Err(answer) => return answer,
      };
      let body = json!({ "supervisors": input.supervisors });
      let path = format!("/rooms/{}/supervisors", input.room_id);

      receiver
        .answered(client.put(format!("{}{path}", receiver.base)).json(&body), &path)
        .await
    }
    other => Answer::failure(format!("this executable holds no entry {other}")),
  }
}

/// One request answered: a call of an entry by its full name.
pub async fn answer(handed: Handed) -> Result<Value, String> {
  let answered = match handed.asked {
    Asked::Call {
      entry: asked,
      arguments,
    } => match asked.strip_prefix("bmo/") {
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

//! The receiver's OpenAPI 3.1 document (`docs/openapi.json`): every route of the fixed API, in the common wire
//! format. The bodies' schemas are generated from this crate's own types; the TypeScript package is generated from
//! this document.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use schemars::Schema;
use schemars::generate::{SchemaGenerator, SchemaSettings};
use serde_json::{Map, Value, json};

use crate::types::*;
use crate::wire::{Page, WireError};

/// Where the schemas the routes refer to stand in the document.
const SCHEMAS: &str = "/components/schemas";

const DESCRIPTION: &str = "The chat receiver: the rooms and their messages, the rules, rights and windows of each \
  room, the envelope a turn goes out in, and the waking of the agents a room's words, a quiet instruction or a \
  job's end calls for. Every route answers in the common wire format: camelCase keys, times as integers of epoch \
  milliseconds, rooms, messages and people as signed 64-bit integers, lists as pages of items and more, and every \
  failure as a non-2xx answer carrying code and error. The routes a chat provider hands messages over on stand \
  under /chat, as bmo-chat's protocol states them.";

/// One route as the document states it.
struct Route {
  id: &'static str,
  tag: &'static str,
  summary: &'static str,
  parameters: Vec<Value>,
  body: Option<Schema>,
  answers: Vec<(u16, Option<Value>)>,
  codes: Vec<&'static str>,
}

fn parameter(name: &str, kind: &str, required: bool) -> Value {
  json!({
    "name": name,
    "in": "query",
    "required": required,
    "schema": { "type": kind },
  })
}

fn path_parameter(name: &str) -> Value {
  json!({
    "name": name,
    "in": "path",
    "required": true,
    "schema": { "type": "integer", "format": "int64" },
  })
}

fn name_parameter(name: &str) -> Value {
  json!({
    "name": name,
    "in": "path",
    "required": true,
    "schema": { "type": "string" },
  })
}

fn json_answer<Answer: JsonSchema>(generator: &mut SchemaGenerator) -> Option<Value> {
  Some(json!({ "application/json": { "schema": generator.subschema_for::<Answer>() } }))
}

fn bytes_answer() -> Option<Value> {
  Some(json!({ "application/octet-stream": { "schema": { "type": "string", "format": "binary" } } }))
}

fn empty_answer() -> Option<Value> {
  None
}

impl Route {
  fn operation(self, error: &Schema) -> Value {
    let mut responses = Map::new();

    for (status, content) in self.answers {
      let description = match status {
        201 => "Created.",
        204 => "Done; no content.",
        _ => "Answered.",
      };
      let mut answer = json!({ "description": description });
      if let Some(content) = content {
        answer["content"] = content;
      }
      responses.insert(status.to_string(), answer);
    }

    if !self.codes.is_empty() {
      let codes = self.codes.join(", ");
      responses.insert(
        "default".to_owned(),
        json!({
          "description": format!("A refusal or failure: {}", codes),
          "content": { "application/json": { "schema": error } },
        }),
      );
    }

    let mut operation = json!({
      "operationId": self.id,
      "tags": [self.tag],
      "summary": self.summary,
      "responses": responses,
      "x-codes": self.codes,
    });
    if !self.parameters.is_empty() {
      operation["parameters"] = self.parameters.into();
    }
    if let Some(body) = self.body {
      operation["requestBody"] = json!({
        "required": true,
        "content": { "application/json": { "schema": body } },
      });
    }

    operation
  }
}

/// The document as JSON.
pub fn openapi() -> Value {
  let mut generator = SchemaSettings::draft2020_12()
    .for_serialize()
    .with(|settings| {
      settings.definitions_path = SCHEMAS.into();
      settings.meta_schema = None;
    })
    .into_generator();
  let error: Schema = generator.subschema_for::<WireError>();
  let route = |item: Route| item.operation(&error);
  let mut paths: BTreeMap<String, Value> = BTreeMap::new();

  macro_rules! on {
    ($method:literal, $path:literal, $id:literal, $tag:literal, $summary:literal, { params: $params:expr, body: $body:expr, answers: $answers:expr, codes: $codes:expr }) => {{
      let item = Route {
        id: $id,
        tag: $tag,
        summary: $summary,
        parameters: $params,
        body: $body,
        answers: $answers,
        codes: $codes,
      };
      route(item)
    }};
  }

  let put = |paths: &mut BTreeMap<String, Value>, path: &str, method: &str, operation: Value| {
    let entry = paths.entry(path.to_owned()).or_insert_with(|| json!({}));
    entry[method.to_lowercase()] = operation;
  };

  macro_rules! add {
    ($method:literal, $path:literal, $id:literal, $tag:literal, $summary:literal, $params:expr, $body:expr, $answers:expr, $codes:expr) => {{
      let operation = on!($method, $path, $id, $tag, $summary, {
        params: $params,
        body: $body,
        answers: $answers,
        codes: $codes
      });
      put(&mut paths, $path, $method, operation);
    }};
  }

  add!(
    "GET",
    "/health",
    "health",
    "Health",
    "Whether the receiver is ready, why not where it is not, and its version.",
    vec![],
    None,
    vec![(200, json_answer::<Health>(&mut generator))],
    vec!["bad-request", "unauthorized", "failed"]
  );
  add!(
    "GET",
    "/owner",
    "owner",
    "Rooms",
    "The deployment's owner as a sender, and the owner's private room.",
    vec![],
    None,
    vec![(200, json_answer::<Owner>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/rooms",
    "rooms",
    "Rooms",
    "Every room the record shows, the pinned ones first.",
    vec![],
    None,
    vec![(200, json_answer::<Page<Room>>(&mut generator))],
    vec!["unauthorized", "upstream-failed", "failed"]
  );
  add!(
    "POST",
    "/rooms",
    "addRoom",
    "Rooms",
    "A room added on a chat service in a mode, one of its account's rooms or the room a conversation it starts comes to, kept with its title and its providers set to that service and the record.",
    vec![],
    Some(generator.subschema_for::<RoomAddition>()),
    vec![(201, json_answer::<Room>(&mut generator))],
    vec!["unauthorized", "bad-request", "conflict", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}",
    "room",
    "Rooms",
    "One room.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<Room>(&mut generator))],
    vec!["unauthorized", "not-found", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/registration",
    "roomRegistration",
    "Rooms",
    "What the receiver holds of a room's registration: its mode, its providers, its answerers, and whether it is read-only and by which chat services.",
    vec![path_parameter("roomId"), parameter("by", "string", true)],
    None,
    vec![(200, json_answer::<RoomRegistration>(&mut generator))],
    vec!["unauthorized", "not-administrator", "not-found", "failed"]
  );
  add!(
    "PUT",
    "/rooms/{roomId}/registration",
    "changeRoomMode",
    "Rooms",
    "A room's mode changed between record-only and avatar, its record, providers and messages untouched.",
    vec![
      path_parameter("roomId"),
      parameter("by", "string", true),
      parameter("account", "integer", false)
    ],
    Some(generator.subschema_for::<ChangeRoomMode>()),
    vec![(200, json_answer::<RoomRegistration>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "not-administrator",
      "not-found",
      "conflict",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "GET",
    "/rooms/{roomId}/leaders",
    "roomLeaders",
    "Rooms",
    "The accounts that lead a room, owning a supervisor it names.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<RoomLeaders>(&mut generator))],
    vec!["unauthorized", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/archive",
    "roomArchive",
    "Rooms",
    "Whether a room is archived and locked, when and by whom.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<RoomArchive>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "PUT",
    "/rooms/{roomId}/archive",
    "changeRoomArchive",
    "Rooms",
    "A room archived and locked, or opened again, by the administrator, the room's leader or a head of the leader's unit.",
    vec![
      path_parameter("roomId"),
      parameter("by", "string", true),
      parameter("account", "integer", false)
    ],
    Some(generator.subschema_for::<ChangeRoomArchive>()),
    vec![(200, json_answer::<RoomArchive>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "not-room-leader",
      "not-found",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "GET",
    "/rooms/{roomId}/title",
    "roomTitle",
    "Rooms",
    "A room's name.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<RoomTitle>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "PUT",
    "/rooms/{roomId}/title",
    "changeRoomTitle",
    "Rooms",
    "A room on the record alone retitled; a room a chat service carries keeps its service's title.",
    vec![path_parameter("roomId")],
    Some(generator.subschema_for::<ChangeRoomTitle>()),
    vec![(200, json_answer::<RoomTitle>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "not-found",
      "private-room",
      "titled-by-service",
      "failed"
    ]
  );
  add!(
    "GET",
    "/rooms/{roomId}/answerers",
    "roomAnswerers",
    "Rooms",
    "The default answerer, the voice and every agent answering a room.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<Answerers>(&mut generator))],
    vec!["unauthorized", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/answerers",
    "answerers",
    "Rooms",
    "Every room with its agents answering it, or one room's where named.",
    vec![parameter("room", "integer", false)],
    None,
    vec![(200, json_answer::<Page<RoomAnswerers>>(&mut generator))],
    vec!["unauthorized", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/messages",
    "messages",
    "Messages",
    "A room's messages, by a cursor and a limit, those carrying a text, or an agent's unread or marked ones.",
    vec![
      path_parameter("roomId"),
      parameter("before", "integer", false),
      parameter("after", "integer", false),
      parameter("limit", "integer", false),
      parameter("agentName", "string", false),
      parameter("since", "integer", false),
      parameter("marked", "boolean", false),
      parameter("unread", "boolean", false),
      parameter("by", "string", false),
      parameter("q", "string", false)
    ],
    None,
    vec![(200, json_answer::<Page<Message>>(&mut generator))],
    vec!["unauthorized", "not-found", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/messages/{messageId}",
    "message",
    "Messages",
    "One message.",
    vec![path_parameter("roomId"), path_parameter("messageId")],
    None,
    vec![(200, json_answer::<Message>(&mut generator))],
    vec!["unauthorized", "not-found", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/messages/{messageId}/metadata",
    "messageMetadata",
    "Messages",
    "What the chat service handed over beside a message.",
    vec![path_parameter("roomId"), path_parameter("messageId")],
    None,
    vec![(200, json_answer::<MessageMetadata>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/messages/{messageId}/sender-is-owner",
    "senderIsOwner",
    "Messages",
    "Whether the message's sender is the owner in that room.",
    vec![path_parameter("roomId"), path_parameter("messageId")],
    None,
    vec![(200, json_answer::<SenderOwner>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/messages/{messageId}/files",
    "messageFiles",
    "Messages",
    "A message's files, by their place.",
    vec![path_parameter("roomId"), path_parameter("messageId")],
    None,
    vec![(200, json_answer::<Page<FileEntry>>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/messages/{messageId}/files/{place}",
    "messageFile",
    "Messages",
    "The bytes of one of a message's files.",
    vec![
      path_parameter("roomId"),
      path_parameter("messageId"),
      name_parameter("place"),
      parameter("download", "boolean", false)
    ],
    None,
    vec![(200, bytes_answer()), (206, bytes_answer())],
    vec!["unauthorized", "not-found"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/messages/{messageId}/picture",
    "messagePicture",
    "Messages",
    "The bytes of a message's picture.",
    vec![path_parameter("roomId"), path_parameter("messageId")],
    None,
    vec![(200, bytes_answer())],
    vec!["unauthorized", "not-found"]
  );
  add!(
    "POST",
    "/rooms/{roomId}/file-fills",
    "fillFiles",
    "Messages",
    "A bounded pass over a room's placeholder rows that kept no file, asking its chat service for their files again, each row asked once.",
    vec![path_parameter("roomId"), parameter("account", "integer", false)],
    Some(generator.subschema_for::<FileFill>()),
    vec![(200, json_answer::<FileFills>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "room-fetches-no-files",
      "room-files-filling",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "GET",
    "/rooms/{roomId}/day-start",
    "dayStart",
    "Messages",
    "The first message a person said at or after a moment.",
    vec![path_parameter("roomId"), parameter("from", "integer", true)],
    None,
    vec![(200, json_answer::<Value>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/avatars",
    "avatars",
    "Rooms",
    "The profile pictures kept, oldest taken first.",
    vec![parameter("after", "integer", false), parameter("limit", "integer", false)],
    None,
    vec![(200, json_answer::<Avatars>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/avatar",
    "roomAvatar",
    "Rooms",
    "The bytes of a room's kept profile picture.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, bytes_answer())],
    vec!["unauthorized", "not-found"]
  );
  add!(
    "GET",
    "/people/{personId}/avatar",
    "personAvatar",
    "Rooms",
    "The bytes of a person's kept profile picture.",
    vec![path_parameter("personId")],
    None,
    vec![(200, bytes_answer())],
    vec!["unauthorized", "not-found"]
  );
  add!(
    "PUT",
    "/avatars/{kind}/{subject}",
    "setAvatar",
    "Rooms",
    "A picture set from the console on a room, a person or an agent, as a form with one picture.",
    vec![
      name_parameter("kind"),
      name_parameter("subject"),
      parameter("account", "integer", false)
    ],
    Some(generator.subschema_for::<Value>()),
    vec![(200, json_answer::<Value>(&mut generator))],
    vec![
      "unauthorized",
      "picture-forbidden",
      "unknown-kind",
      "no-picture-given",
      "not-a-picture",
      "picture-too-large",
      "no-number",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "DELETE",
    "/avatars/{kind}/{subject}",
    "unsetAvatar",
    "Rooms",
    "A picture set from the console let go, so the one its chat service handed over stands.",
    vec![
      name_parameter("kind"),
      name_parameter("subject"),
      parameter("account", "integer", false)
    ],
    None,
    vec![(200, json_answer::<Value>(&mut generator))],
    vec![
      "unauthorized",
      "picture-forbidden",
      "unknown-kind",
      "no-number",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "GET",
    "/rooms/{roomId}/pictures",
    "pictures",
    "Rooms",
    "A room's pictures, walked from a picture the reader holds.",
    vec![
      path_parameter("roomId"),
      parameter("at", "integer", false),
      parameter("before", "integer", false),
      parameter("after", "integer", false),
      parameter("limit", "integer", false)
    ],
    None,
    vec![(200, json_answer::<Pictures>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/figures",
    "figures",
    "Rooms",
    "A room's figures, by hour and agent.",
    vec![path_parameter("roomId"), parameter("zone", "string", false)],
    None,
    vec![(200, json_answer::<Figures>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/window",
    "window",
    "Rooms",
    "How long the default answerer's window in a room has left.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<Window>(&mut generator))],
    vec!["unauthorized", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/situation",
    "situation",
    "Rooms",
    "A room's situation note as it stands, with when it last changed.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<RoomSituation>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/providers",
    "providers",
    "Rooms",
    "Every chat provider a room may be on.",
    vec![],
    None,
    vec![(200, json_answer::<Page<Provider>>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/providers/{key}/rooms",
    "accountRooms",
    "Rooms",
    "The rooms a provider's account is in, relayed from its own list, each with whether the receiver holds it and the mode it was added in.",
    vec![
      name_parameter("key"),
      parameter("after", "integer", false),
      parameter("limit", "integer", false)
    ],
    None,
    vec![(200, json_answer::<Page<AccountRoom>>(&mut generator))],
    vec!["unauthorized", "bad-request", "not-found", "conflict", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/providers/{key}/contacts",
    "accountContacts",
    "Rooms",
    "The contacts of a provider's account, or of the account `accountId` names, relayed from its own list, each with its one-to-one, whether the receiver holds it and the mode it was added in.",
    vec![
      name_parameter("key"),
      parameter("after", "integer", false),
      parameter("limit", "integer", false),
      parameter("accountId", "integer", false)
    ],
    None,
    vec![(200, json_answer::<Page<AccountContact>>(&mut generator))],
    vec!["unauthorized", "bad-request", "not-found", "conflict", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/providers",
    "roomProviders",
    "Rooms",
    "The chat providers a room is on, and whether its list was set.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<RoomProviders>(&mut generator))],
    vec!["unauthorized", "not-found", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/members",
    "roomMembers",
    "Rooms",
    "The people in a room as its own chat service lists them, each by the sender number the room's messages carry.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<RoomMembers>(&mut generator))],
    vec![
      "unauthorized",
      "not-found",
      "record-only-room",
      "members-unlisted",
      "members-unheld",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "PUT",
    "/rooms/{roomId}/providers",
    "chooseProviders",
    "Rooms",
    "A room's chat providers set whole.",
    vec![path_parameter("roomId"), parameter("account", "integer", false)],
    Some(generator.subschema_for::<ChooseProviders>()),
    vec![(200, json_answer::<RoomProviders>(&mut generator))],
    vec!["unauthorized", "bad-request", "not-found", "private-room", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/roads",
    "road",
    "Rooms",
    "A room's road: the owner's own or a group, and whether a one-to-one.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<Road>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/own-number",
    "ownNumber",
    "Rooms",
    "The sender an agent's own words stand under in a room.",
    vec![path_parameter("roomId"), parameter("agentName", "string", true)],
    None,
    vec![(200, json_answer::<OwnNumber>(&mut generator))],
    vec!["unauthorized", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/spoke-between",
    "spokeBetween",
    "People",
    "Whether a person spoke in a room in a span.",
    vec![
      path_parameter("roomId"),
      parameter("person", "integer", true),
      parameter("from", "integer", true),
      parameter("to", "integer", true)
    ],
    None,
    vec![(200, json_answer::<Spoke>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/people",
    "roomPeople",
    "People",
    "Of those asked, the people who have stood as a sender in a room.",
    vec![path_parameter("roomId"), parameter("ids", "string", true)],
    None,
    vec![(200, json_answer::<Page<Person>>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/owner-spoke",
    "ownerSpoke",
    "People",
    "Whether the person is the owner, and whether the owner spoke in the span.",
    vec![
      path_parameter("roomId"),
      parameter("person", "integer", true),
      parameter("before", "integer", true),
      parameter("windowMs", "integer", true)
    ],
    None,
    vec![(200, json_answer::<OwnerSpoke>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/read-marks",
    "readMarks",
    "Read marks",
    "How many of a room's messages an agent has marked.",
    vec![path_parameter("roomId"), parameter("agentName", "string", true)],
    None,
    vec![(200, json_answer::<ReadMarkCount>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "POST",
    "/rooms/{roomId}/read-marks",
    "keepMarks",
    "Read marks",
    "The room's messages marked as read into an agent's book.",
    vec![path_parameter("roomId"), parameter("account", "integer", false)],
    Some(generator.subschema_for::<KeepMarks>()),
    vec![(204, empty_answer())],
    vec!["unauthorized", "bad-request", "private-room", "upstream-failed", "failed"]
  );
  add!(
    "DELETE",
    "/rooms/{roomId}/read-marks",
    "forgetMarks",
    "Read marks",
    "Every mark of an agent's book let go, off one room.",
    vec![
      path_parameter("roomId"),
      parameter("agentName", "string", true),
      parameter("account", "integer", false)
    ],
    None,
    vec![(200, json_answer::<Forgotten>(&mut generator))],
    vec!["unauthorized", "private-room", "upstream-failed", "failed"]
  );
  add!(
    "POST",
    "/read-mark-counts",
    "readMarkCounts",
    "Read marks",
    "Every room holding messages past an agent's marks, each with how many.",
    vec![],
    Some(generator.subschema_for::<ReadMarkCounts>()),
    vec![(200, json_answer::<Page<RoomReadCount>>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/notices",
    "notices",
    "Notices",
    "The notices a room was voiced, by kind and span.",
    vec![
      path_parameter("roomId"),
      parameter("before", "integer", false),
      parameter("after", "integer", false),
      parameter("limit", "integer", false),
      parameter("kind", "string", false),
      parameter("from", "integer", false),
      parameter("to", "integer", false)
    ],
    None,
    vec![(200, json_answer::<Page<Notice>>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "POST",
    "/rooms/{roomId}/notices",
    "notice",
    "Notices",
    "A notice queued for the room's voice.",
    vec![path_parameter("roomId"), parameter("account", "integer", false)],
    Some(generator.subschema_for::<Value>()),
    vec![(201, json_answer::<Created>(&mut generator))],
    vec!["unauthorized", "bad-request", "private-room", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/rooms/{roomId}/send-verbs",
    "sendVerbs",
    "Sends",
    "The verbs of the road a send naming `via` goes out on in the room; one provider's own are its `can` on `GET /providers`.",
    vec![path_parameter("roomId"), parameter("via", "string", false)],
    None,
    vec![(200, json_answer::<SendVerbs>(&mut generator))],
    vec!["unauthorized", "conflict", "failed"]
  );
  add!(
    "GET",
    "/people",
    "people",
    "People",
    "Each person asked with the name they last went by.",
    vec![parameter("ids", "string", true)],
    None,
    vec![(200, json_answer::<Page<Person>>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "POST",
    "/promise-readings",
    "promiseReading",
    "Readings",
    "Whether a text waits for something to happen first.",
    vec![],
    Some(generator.subschema_for::<PromiseAsk>()),
    vec![(200, json_answer::<PromiseReading>(&mut generator))],
    vec!["unauthorized", "bad-request"]
  );
  add!(
    "POST",
    "/instructions",
    "queueInstruction",
    "Instructions",
    "An instruction queued for an agent's session in a room.",
    vec![],
    Some(generator.subschema_for::<InstructionRequest>()),
    vec![(201, json_answer::<Created>(&mut generator))],
    vec!["unauthorized", "bad-request", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/instructions/{instructionId}",
    "instruction",
    "Instructions",
    "How far the instruction named has gone.",
    vec![path_parameter("instructionId")],
    None,
    vec![(200, json_answer::<Instruction>(&mut generator))],
    vec!["unauthorized", "not-found", "failed"]
  );
  add!(
    "DELETE",
    "/instructions/{instructionId}",
    "withdrawInstruction",
    "Instructions",
    "The instruction not yet taken withdrawn.",
    vec![path_parameter("instructionId")],
    None,
    vec![(204, empty_answer())],
    vec!["unauthorized", "failed"]
  );
  add!(
    "POST",
    "/owner/notices",
    "ownerNotice",
    "Notices",
    "A code and its parameters queued for the default answerer in the owner's private chat.",
    vec![],
    Some(generator.subschema_for::<OwnerNotice>()),
    vec![(201, json_answer::<Created>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "POST",
    "/owner/instructions",
    "ownerInstruction",
    "Instructions",
    "The owner's words queued for a room's session.",
    vec![parameter("account", "integer", false)],
    Some(generator.subschema_for::<OwnerInstruction>()),
    vec![(204, empty_answer())],
    vec![
      "unauthorized",
      "bad-request",
      "not-found",
      "forbidden",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "GET",
    "/rooms/{roomId}/accounts",
    "roomAccounts",
    "Rooms",
    "A room's members by account in the console; a private room's own account alone.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<RoomAccounts>(&mut generator))],
    vec!["unauthorized", "not-found", "upstream-failed", "failed"]
  );
  add!(
    "PUT",
    "/rooms/{roomId}/accounts/{account}",
    "letIn",
    "Rooms",
    "An account let into a room by another, with a line saying so; the member as it stands where it already is one.",
    vec![path_parameter("roomId"), path_parameter("account")],
    Some(generator.subschema_for::<LetIn>()),
    vec![(200, json_answer::<RoomAccount>(&mut generator))],
    vec!["unauthorized", "bad-request", "not-found", "private-room", "upstream-failed", "failed"]
  );
  add!(
    "DELETE",
    "/rooms/{roomId}/accounts/{account}",
    "takeOut",
    "Rooms",
    "A member taken out of a room, or leaving it where by is itself, with a line saying so.",
    vec![path_parameter("roomId"), path_parameter("account"), parameter("by", "integer", true)],
    None,
    vec![(204, empty_answer())],
    vec!["unauthorized", "bad-request", "not-found", "private-room", "upstream-failed", "failed"]
  );
  add!(
    "PUT",
    "/accounts/{account}/identity",
    "accountIdentity",
    "Rooms",
    "An account's record identity, made where it does not stand and the same one ever after.",
    vec![path_parameter("account")],
    None,
    vec![(200, json_answer::<AccountIdentity>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "GET",
    "/accounts/{account}/rooms",
    "accountMemberships",
    "Rooms",
    "The rooms an account is a member of in the console now, its private rooms among them.",
    vec![path_parameter("account")],
    None,
    vec![(200, json_answer::<AccountMemberships>(&mut generator))],
    vec!["unauthorized", "bad-request", "upstream-failed", "failed"]
  );
  add!(
    "POST",
    "/rooms/{roomId}/words",
    "accountWords",
    "Sends",
    "Words an account said in a room in the console, kept under the owner's sender for the owner's account and under the account's record identity for any other.",
    vec![path_parameter("roomId"), parameter("account", "integer", false)],
    Some(generator.subschema_for::<AccountWords>()),
    vec![(201, json_answer::<Created>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "no-route",
      "conflict",
      "room-archived",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "POST",
    "/rooms/{roomId}/word-files",
    "accountWordFiles",
    "Sends",
    "Files an account sent into a room in the console, as a form: the owner's files form with account and name in place of chatId.",
    vec![path_parameter("roomId"), parameter("account", "integer", false)],
    Some(generator.subschema_for::<Value>()),
    vec![(200, json_answer::<Value>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "no-route",
      "room-archived",
      "private-room",
      "too-large",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "PUT",
    "/room-holders/{holderName}",
    "declareHolder",
    "Rooms",
    "A unit declared a holder of rows naming a room.",
    vec![name_parameter("holderName")],
    Some(generator.subschema_for::<RoomHolder>()),
    vec![(204, empty_answer())],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "POST",
    "/sends",
    "send",
    "Sends",
    "A send carried as its asker in its own round, or handed to the room's supervisor, answered with its lines or its refusal.",
    vec![],
    Some(generator.subschema_for::<SendRequest>()),
    vec![(200, json_answer::<SendAnswer>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "POST",
    "/owner/messages",
    "ownerMessage",
    "Sends",
    "The owner's words typed into a room on a screen.",
    vec![parameter("account", "integer", false)],
    Some(generator.subschema_for::<OwnerMessage>()),
    vec![(200, json_answer::<Created>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "not-found",
      "conflict",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "GET",
    "/rooms/{roomId}/pins",
    "pins",
    "Messages",
    "A room's pinned messages, newest pin first.",
    vec![path_parameter("roomId")],
    None,
    vec![(200, json_answer::<Pins>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "PUT",
    "/rooms/{roomId}/messages/{messageId}/reactions/mine",
    "reactAsMe",
    "Messages",
    "The signed-in person's reaction put on a message.",
    vec![
      path_parameter("roomId"),
      path_parameter("messageId"),
      parameter("account", "integer", false)
    ],
    Some(generator.subschema_for::<Reacting>()),
    vec![(200, json_answer::<Message>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "no-caller",
      "message-unknown",
      "message-deleted",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "DELETE",
    "/rooms/{roomId}/messages/{messageId}/reactions/mine",
    "unreactAsMe",
    "Messages",
    "The signed-in person's reaction taken off a message.",
    vec![
      path_parameter("roomId"),
      path_parameter("messageId"),
      parameter("account", "integer", false)
    ],
    None,
    vec![(200, json_answer::<Message>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "no-caller",
      "message-unknown",
      "message-deleted",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "PUT",
    "/rooms/{roomId}/messages/{messageId}/pin",
    "pinAsMe",
    "Messages",
    "A message pinned or unpinned by the signed-in person.",
    vec![
      path_parameter("roomId"),
      path_parameter("messageId"),
      parameter("account", "integer", false)
    ],
    Some(generator.subschema_for::<Pinning>()),
    vec![(200, json_answer::<Message>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "no-caller",
      "message-unknown",
      "message-deleted",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "PATCH",
    "/rooms/{roomId}/messages/{messageId}",
    "editAsMe",
    "Messages",
    "The signed-in person's own message edited.",
    vec![
      path_parameter("roomId"),
      path_parameter("messageId"),
      parameter("account", "integer", false)
    ],
    Some(generator.subschema_for::<Editing>()),
    vec![(200, json_answer::<Message>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "no-caller",
      "message-unknown",
      "message-deleted",
      "message-forbidden",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "PUT",
    "/rooms/{roomId}/messages/{messageId}/speaker",
    "changeSpeaker",
    "Messages",
    "A line under an agent's own number in the room filed as that agent's, on the record alone.",
    vec![path_parameter("roomId"), path_parameter("messageId")],
    Some(generator.subschema_for::<ChangeSpeaker>()),
    vec![(200, json_answer::<Message>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "message-unknown",
      "message-not-agents",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "DELETE",
    "/rooms/{roomId}/messages/{messageId}",
    "deleteAsMe",
    "Messages",
    "The signed-in person's own message deleted, or any by the administrator outside another account's private room.",
    vec![
      path_parameter("roomId"),
      path_parameter("messageId"),
      parameter("account", "integer", false)
    ],
    None,
    vec![(200, json_answer::<Message>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "no-caller",
      "message-unknown",
      "message-deleted",
      "message-forbidden",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "GET",
    "/stickers",
    "stickers",
    "Messages",
    "The stickers the record keeps, each by its emoji.",
    vec![],
    None,
    vec![(200, json_answer::<Stickers>(&mut generator))],
    vec!["unauthorized"]
  );
  add!(
    "POST",
    "/rooms/{roomId}/stickers",
    "stickerAsMe",
    "Messages",
    "A sticker of the record's set sent as the signed-in person.",
    vec![path_parameter("roomId"), parameter("account", "integer", false)],
    Some(generator.subschema_for::<StickerSend>()),
    vec![(201, json_answer::<Message>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "no-caller",
      "unknown-sticker",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "POST",
    "/rooms/{roomId}/messages/{messageId}/forwards",
    "forward",
    "Sends",
    "A message forwarded into another room, kept there as the forwarding account's, the owner's where the body names none, and marked as forwarded.",
    vec![
      path_parameter("roomId"),
      path_parameter("messageId"),
      parameter("account", "integer", false)
    ],
    Some(generator.subschema_for::<Forward>()),
    vec![(201, json_answer::<Value>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "not-found",
      "conflict",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "POST",
    "/owner/message-files",
    "ownerMessageFiles",
    "Sends",
    "The owner's files sent into a room on a screen, as a form.",
    vec![parameter("account", "integer", false)],
    Some(generator.subschema_for::<Value>()),
    vec![(200, json_answer::<Value>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "not-found",
      "too-large",
      "private-room",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "POST",
    "/consult-lines",
    "consultLine",
    "Consult",
    "A line said in the consult room under an agent's own name, or handed to that agent outside its own round.",
    vec![],
    Some(generator.subschema_for::<ConsultLine>()),
    vec![(200, json_answer::<ConsultSaid>(&mut generator))],
    vec!["unauthorized", "bad-request", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/consult-lines",
    "consultBlock",
    "Consult",
    "The block an agent is shown of the other agents' work.",
    vec![parameter("agentName", "string", true)],
    None,
    vec![(200, json_answer::<ConsultBlock>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "POST",
    "/talks",
    "talk",
    "Talks",
    "A talk the owner has with the default answerer about a topic it names.",
    vec![],
    Some(generator.subschema_for::<Talk>()),
    vec![(200, json_answer::<TalkAnswer>(&mut generator))],
    vec!["unauthorized", "bad-request", "conflict", "not-ready", "failed"]
  );
  add!(
    "POST",
    "/handoffs/{agentName}",
    "handOff",
    "Handoffs",
    "An agent's standing session asked for its handoff file ahead of a fresh session, kept for that session's first turn.",
    vec![name_parameter("agentName")],
    Some(generator.subschema_for::<HandoffAsked>()),
    vec![(200, json_answer::<HandoffWritten>(&mut generator)), (204, empty_answer())],
    vec!["unauthorized", "bad-request", "upstream-failed"]
  );
  add!(
    "POST",
    "/handovers/{agentName}",
    "handOver",
    "Handovers",
    "An agent's one session asked to hand over at its next turn: it writes its handoff note and a fresh session opens on it.",
    vec![name_parameter("agentName")],
    Some(generator.subschema_for::<HandoverAsked>()),
    vec![(202, json_answer::<HandoverPending>(&mut generator))],
    vec!["unauthorized", "bad-request", "not-found", "conflict", "upstream-failed"]
  );
  add!(
    "GET",
    "/voice-briefing",
    "voiceBriefing",
    "Voice briefing",
    "The voice briefing's two switches as they stand.",
    vec![],
    None,
    vec![(200, json_answer::<VoiceBriefing>(&mut generator))],
    vec!["unauthorized"]
  );
  add!(
    "PUT",
    "/voice-briefing",
    "switchVoiceBriefing",
    "Voice briefing",
    "One of the voice briefing's switches moved; moving the whole briefing is told to the owner, and setting it off stops every briefing the voice engine holds.",
    vec![],
    Some(generator.subschema_for::<VoiceBriefingSwitch>()),
    vec![(200, json_answer::<VoiceBriefing>(&mut generator))],
    vec!["unauthorized", "one-switch", "upstream-failed"]
  );
  add!(
    "POST",
    "/rooms/{roomId}/messages/{messageId}/reading",
    "readLine",
    "Voice briefing",
    "A line's full reading, launched in the voice engine where none stands, answered as it stands there otherwise.",
    vec![
      path_parameter("roomId"),
      path_parameter("messageId"),
      parameter("account", "integer", false)
    ],
    None,
    vec![(200, json_answer::<LineReading>(&mut generator))],
    vec![
      "unauthorized",
      "bad-request",
      "not-found",
      "switched-off",
      "voiceless",
      "unreachable",
      "private-room",
      "upstream-failed"
    ]
  );
  add!(
    "POST",
    "/events",
    "publishEvent",
    "Events",
    "An event a process beside the receiver publishes.",
    vec![],
    Some(generator.subschema_for::<PublishedEvent>()),
    vec![(204, empty_answer())],
    vec!["unauthorized", "bad-request", "not-found"]
  );
  add!(
    "GET",
    "/events",
    "events",
    "Events",
    "The receiver's one stream, as server-sent events.",
    vec![
      parameter("after", "string", false),
      parameter("run", "string", false),
      parameter("kinds", "string", false),
      parameter("roomId", "integer", false)
    ],
    None,
    vec![(200, Some(json!({ "text/event-stream": { "schema": { "type": "string" } } })))],
    vec!["unauthorized"]
  );
  add!(
    "GET",
    "/live",
    "live",
    "Events",
    "The picture of what the process runs, asked for once.",
    vec![],
    None,
    vec![(200, json_answer::<Live>(&mut generator))],
    vec!["unauthorized"]
  );
  add!(
    "POST",
    "/hooks/{point}",
    "hook",
    "Hooks",
    "A hook the agency calls at a point of its work.",
    vec![name_parameter("point")],
    Some(generator.subschema_for::<Value>()),
    vec![(200, json_answer::<HookAnswer>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "DELETE",
    "/rooms/{roomId}",
    "removeRoom",
    "Rooms",
    "A room's registration removed, or with dryRun what the removal would do.",
    vec![
      path_parameter("roomId"),
      parameter("by", "string", true),
      parameter("dryRun", "boolean", false)
    ],
    None,
    vec![(200, json_answer::<Value>(&mut generator))],
    vec!["unauthorized", "forbidden", "not-found", "private-room", "upstream-failed", "failed"]
  );
  add!(
    "GET",
    "/rounds/openings",
    "roundOpenings",
    "Rounds",
    "For a room and the rounds named, what each settled.",
    vec![parameter("room", "integer", true), parameter("rounds", "string", false)],
    None,
    vec![(200, json_answer::<Page<RoundOpening>>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/rounds/openings/{instructionId}",
    "roundOpening",
    "Rounds",
    "One quiet instruction's whole text.",
    vec![path_parameter("instructionId"), parameter("room", "integer", true)],
    None,
    vec![(200, json_answer::<OpeningText>(&mut generator))],
    vec!["unauthorized", "not-found", "failed"]
  );
  add!(
    "GET",
    "/rounds/drafts",
    "roundDrafts",
    "Rounds",
    "For a room and the rounds named, or its newest rounds where none is named, every row each round's draft process wrote.",
    vec![
      parameter("room", "integer", true),
      parameter("rounds", "string", false),
      parameter("limit", "integer", false),
    ],
    None,
    vec![(200, json_answer::<Page<RoundDrafts>>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "GET",
    "/draft-processes",
    "draftProcesses",
    "Drafts",
    "Every row of the draft process table, the policy's and each supervisor's own; for a supervisor named, the one row it reads.",
    vec![parameter("agentName", "string", false)],
    None,
    vec![(200, json_answer::<Page<DraftProcess>>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "PUT",
    "/draft-processes/{provider}/{tier}/{agentName}",
    "setDraftProcess",
    "Drafts",
    "A supervisor's row set by the console.",
    vec![name_parameter("provider"), name_parameter("tier"), name_parameter("agentName")],
    Some(generator.subschema_for::<DraftProcessSet>()),
    vec![(200, json_answer::<DraftProcess>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "DELETE",
    "/draft-processes/{provider}/{tier}/{agentName}",
    "removeDraftProcess",
    "Drafts",
    "A supervisor's row taken away.",
    vec![name_parameter("provider"), name_parameter("tier"), name_parameter("agentName")],
    None,
    vec![(204, empty_answer())],
    vec!["unauthorized", "not-found", "failed"]
  );
  add!(
    "GET",
    "/draft-questions",
    "draftQuestions",
    "Drafts",
    "A supervisor's catalogue of questions, or the policy's alone where none is named.",
    vec![parameter("agentName", "string", false)],
    None,
    vec![(200, json_answer::<Page<DraftQuestion>>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "PUT",
    "/draft-questions/{agentName}/{name}",
    "setDraftQuestion",
    "Drafts",
    "A question's text set by the console for a supervisor.",
    vec![name_parameter("agentName"), name_parameter("name")],
    Some(generator.subschema_for::<DraftQuestionSet>()),
    vec![(200, json_answer::<DraftQuestion>(&mut generator))],
    vec!["unauthorized", "bad-request", "failed"]
  );
  add!(
    "DELETE",
    "/draft-questions/{agentName}/{name}",
    "removeDraftQuestion",
    "Drafts",
    "The console's text of a supervisor's question taken away.",
    vec![name_parameter("agentName"), name_parameter("name")],
    None,
    vec![(204, empty_answer())],
    vec!["unauthorized", "not-found", "conflict", "failed"]
  );
  add!(
    "POST",
    "/supervisors",
    "addSupervisor",
    "Supervisors",
    "A supervisor added with a room of the console's own.",
    vec![],
    Some(generator.subschema_for::<SupervisorRequest>()),
    vec![(201, json_answer::<SupervisorAdded>(&mut generator))],
    vec!["unauthorized", "bad-request", "conflict", "upstream-failed", "failed"]
  );
  add!(
    "POST",
    "/supervisors/{supervisorName}/resets",
    "resetSupervisor",
    "Supervisors",
    "A supervisor started afresh, handed the rooms that name it alone and beside another.",
    vec![name_parameter("supervisorName")],
    Some(generator.subschema_for::<SupervisorReset>()),
    vec![(200, json_answer::<Value>(&mut generator))],
    vec!["unauthorized", "bad-request", "not-found", "upstream-failed"]
  );
  add!(
    "GET",
    "/supervisors/{supervisorName}/rooms",
    "supervisorRooms",
    "Supervisors",
    "The rooms a supervisor answers: those that name it and, for the default answerer, those naming none.",
    vec![name_parameter("supervisorName")],
    None,
    vec![(200, json_answer::<Page<SupervisorRoom>>(&mut generator))],
    vec!["unauthorized", "failed"]
  );
  add!(
    "GET",
    "/supervisors/{supervisorName}/rooms/{roomId}/messages",
    "supervisorRoomMessages",
    "Supervisors",
    "A group room's messages as a supervisor reads them: the room's answerer, or a member's own supervisor.",
    vec![
      name_parameter("supervisorName"),
      path_parameter("roomId"),
      parameter("before", "integer", false),
      parameter("after", "integer", false),
      parameter("limit", "integer", false)
    ],
    None,
    vec![(200, json_answer::<Page<Message>>(&mut generator))],
    vec!["unauthorized", "not-a-group", "not-a-member", "not-found", "failed"]
  );
  add!(
    "GET",
    "/default-answerer",
    "defaultAnswerer",
    "Supervisors",
    "The supervisor that answers every room naming none.",
    vec![],
    None,
    vec![(200, json_answer::<DefaultAnswerer>(&mut generator))],
    vec!["unauthorized"]
  );
  add!(
    "PUT",
    "/default-answerer",
    "setDefaultAnswerer",
    "Supervisors",
    "The default answerer setting moved to a supervisor the agency registers.",
    vec![],
    Some(generator.subschema_for::<DefaultAnswerer>()),
    vec![(200, json_answer::<DefaultAnswerer>(&mut generator))],
    vec!["unauthorized", "bad-request", "not-found", "upstream-failed", "failed"]
  );
  add!(
    "PUT",
    "/rooms/{roomId}/supervisors",
    "setRoomSupervisors",
    "Rooms",
    "A room's whole list of supervisors; an empty list hands the room to the default answerer.",
    vec![path_parameter("roomId"), parameter("account", "integer", false)],
    Some(generator.subschema_for::<RoomSupervisors>()),
    vec![(200, json_answer::<RoomSupervisors>(&mut generator))],
    vec!["unauthorized", "bad-request", "not-found", "private-room", "upstream-failed", "failed"]
  );
  add!(
    "DELETE",
    "/supervisors/{supervisorName}",
    "removeSupervisor",
    "Supervisors",
    "A supervisor taken out, the rooms naming it handed to the agency as shared; a seeded supervisor, the \
     default answerer and one a turn holds are refused.",
    vec![
      name_parameter("supervisorName"),
      parameter("by", "string", false),
      parameter("forgetBrain", "boolean", false),
      parameter("force", "boolean", false),
    ],
    None,
    vec![(204, empty_answer())],
    vec![
      "unauthorized",
      "forbidden",
      "bad-request",
      "not-found",
      "conflict",
      "upstream-failed",
      "failed"
    ]
  );
  add!(
    "PUT",
    "/pinned-rooms",
    "pinRooms",
    "Rooms",
    "The pinned order set whole.",
    vec![parameter("account", "integer", false)],
    Some(generator.subschema_for::<PinRooms>()),
    vec![(200, json_answer::<PinnedRooms>(&mut generator))],
    vec!["unauthorized", "bad-request", "not-found", "private-room", "upstream-failed", "failed"]
  );
  add!(
    "POST",
    "/pinned-rooms",
    "pinRoom",
    "Rooms",
    "A room stood last in the pinned order.",
    vec![parameter("account", "integer", false)],
    Some(generator.subschema_for::<PinRoom>()),
    vec![(200, json_answer::<PinnedRooms>(&mut generator))],
    vec!["unauthorized", "bad-request", "not-found", "private-room", "upstream-failed", "failed"]
  );
  add!(
    "DELETE",
    "/pinned-rooms/{roomId}",
    "unpinRoom",
    "Rooms",
    "A room taken out of the pinned order.",
    vec![path_parameter("roomId"), parameter("account", "integer", false)],
    None,
    vec![(200, json_answer::<PinnedRooms>(&mut generator))],
    vec!["unauthorized", "bad-request", "private-room", "upstream-failed", "failed"]
  );

  let schemas: Map<String, Value> = generator.take_definitions(true);

  json!({
    "openapi": "3.1.0",
    "info": {
      "title": "bmo-receiver",
      "version": crate::VERSION,
      "description": DESCRIPTION,
    },
    "x-layer": "downstream",
    "x-stage": 5,
    "paths": paths,
    "components": {
      "schemas": schemas,
    },
  })
}

/// The document as `docs/openapi.json` keeps it: pretty, ending in a line break.
pub fn openapi_json() -> String {
  let mut written = serde_json::to_string_pretty(&openapi()).expect("the document is JSON");

  written.push('\n');
  written
}

/// The body `POST /promise-readings` takes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromiseAsk {
  pub text: String,
}

/// The body that marks a room's messages read into an agent's book.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeepMarks {
  pub agent_name: String,
  pub message_ids: Vec<i64>,
}

/// The body that counts the messages past an agent's marks.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadMarkCounts {
  pub agent_name: String,
  #[serde(default)]
  pub since: Vec<RoomSince>,
}

/// One room's moment to count from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomSince {
  pub room: i64,
  /// Epoch milliseconds.
  pub since: i64,
}

//! A send asked of the running receiver: how it is written, who asks it, and what the receiver's answer says
//! of it (docs/chat-mcp.md, 「How a call runs」).

use room_calls::bot::Service;
use room_calls::sends::{
  Act, Asked, Asker, CONSULT_PATH, ConsultInput, FileInput, FileToSend, FixedLineInput, RandomDrawInput,
  SENDS_PATH, StickerInput, Unsent, VoiceInput, answered, ask_consult,
};
use room_calls::turn::{Room, Turn};
use serde_json::{Map, Value, json};

const ROOM: i64 = 4_200;

#[test]
fn an_asked_send_is_written_with_its_tool_its_input_and_its_asker() {
  let asked = Asked {
    asker: Asker {
      room: Some(ROOM),
      agent_name: Some("agent-a".to_owned()),
      by: Some(format!("chat:{ROOM}")),
      ..Asker::default()
    },
    act: Act::Sticker(StickerInput {
      chat: None,
      via: None,
      emoji: ":+1:".to_owned(),
      reply_to: None,
    }),
  };
  let written = serde_json::to_value(&asked).unwrap();

  assert_eq!(
    written,
    json!({
      "asker": { "room": ROOM, "agentName": "agent-a", "by": format!("chat:{ROOM}") },
      "tool": "sticker",
      "input": { "emoji": ":+1:" },
    }),
  );
  assert_eq!(serde_json::from_value::<Asked>(written).unwrap(), asked);
}

#[test]
fn a_file_send_is_written_with_its_files_and_caption() {
  let asked = Asked {
    asker: Asker {
      room: Some(ROOM),
      agent_name: Some("agent-a".to_owned()),
      ..Asker::default()
    },
    act: Act::File(FileInput {
      chat: None,
      via: None,
      files: vec![
        FileToSend {
          path: "/tmp/notes.txt".to_owned(),
          name: None,
        },
        FileToSend {
          path: "/tmp/report.pdf".to_owned(),
          name: Some("report.pdf".to_owned()),
        },
      ],
      caption: Some("the two of them".to_owned()),
      reply_to: None,
    }),
  };
  let written = serde_json::to_value(&asked).unwrap();

  assert_eq!(
    written,
    json!({
      "asker": { "room": ROOM, "agentName": "agent-a" },
      "tool": "file",
      "input": {
        "files": [
          { "path": "/tmp/notes.txt" },
          { "path": "/tmp/report.pdf", "name": "report.pdf" },
        ],
        "caption": "the two of them",
      },
    }),
  );
  assert_eq!(serde_json::from_value::<Asked>(written).unwrap(), asked);
}

#[test]
fn a_voice_send_is_written_with_its_path_its_caption_and_what_it_says() {
  let asked = Asked {
    asker: Asker {
      room: Some(ROOM),
      agent_name: Some("agent-a".to_owned()),
      ..Asker::default()
    },
    act: Act::Voice(VoiceInput {
      chat: None,
      via: None,
      path: "/tmp/note.ogg".to_owned(),
      caption: Some("the news".to_owned()),
      said: Some("today's news".to_owned()),
      transcript: Some("the whole of the news".to_owned()),
      reply_to: None,
    }),
  };
  let written = serde_json::to_value(&asked).unwrap();

  assert_eq!(
    written,
    json!({
      "asker": { "room": ROOM, "agentName": "agent-a" },
      "tool": "voice",
      "input": {
        "path": "/tmp/note.ogg",
        "caption": "the news",
        "said": "today's news",
        "transcript": "the whole of the news",
      },
    }),
  );
  assert_eq!(serde_json::from_value::<Asked>(written).unwrap(), asked);
}

#[test]
fn a_fixed_line_is_written_with_its_code_and_fields() {
  let mut fields = Map::new();

  fields.insert("label".to_owned(), json!("the word chain"));
  fields.insert("result".to_owned(), json!("apple"));

  let asked = Asked {
    asker: Asker {
      room: Some(ROOM),
      agent_name: Some("agent-a".to_owned()),
      ..Asker::default()
    },
    act: Act::FixedLine(FixedLineInput {
      chat: None,
      via: None,
      code: "draw".to_owned(),
      fields,
    }),
  };
  let written = serde_json::to_value(&asked).unwrap();

  assert_eq!(
    written,
    json!({
      "asker": { "room": ROOM, "agentName": "agent-a" },
      "tool": "fixed_line",
      "input": {
        "code": "draw",
        "fields": { "label": "the word chain", "result": "apple" },
      },
    }),
  );
  assert_eq!(serde_json::from_value::<Asked>(written).unwrap(), asked);
}

#[test]
fn a_draws_own_fields_leave_out_the_room_and_the_road() {
  let input: RandomDrawInput = serde_json::from_value(json!({
    "chat": ROOM,
    "via": "road-a",
    "shape": "range",
    "min": 1,
    "max": 6,
  }))
  .unwrap();

  assert_eq!(Value::Object(input.request()), json!({ "shape": "range", "min": 1, "max": 6 }));
}

#[test]
fn a_draw_naming_a_mode_does_not_read() {
  let read =
    serde_json::from_value::<RandomDrawInput>(json!({ "mode": "async", "shape": "dice", "sides": 6 }));

  assert!(read.unwrap_err().to_string().contains("mode"));
}

#[test]
fn the_asker_of_a_turn_sets_the_send_off_from_the_turns_own_chat() {
  let turn = Turn {
    agent: Some("agent-a".to_owned()),
    chat: Some(ROOM),
    via: Some("road-a".to_owned()),
    job: true,
    ..Turn::default()
  };

  assert_eq!(
    Asker::of_turn(&turn, turn.room(Some(-200)).as_ref()),
    Asker {
      room: Some(-200),
      via: None,
      agent_name: Some("agent-a".to_owned()),
      by: Some(format!("chat:{ROOM}")),
      job: true,
    },
  );
  assert_eq!(turn.room(None).unwrap().via.as_deref(), Some("road-a"));
  assert_eq!(Turn::default().room(None), None);
  assert_eq!(
    turn.room(Some(ROOM)).as_ref(),
    Some(&Room {
      chat: ROOM,
      via: Some("road-a".to_owned())
    }),
  );
}

#[test]
fn the_receivers_answer_says_the_lines_the_refusal_or_that_nothing_came_back() {
  assert_eq!(answered(SENDS_PATH, 200, &json!({ "lines": ["sent"] })), Ok(vec!["sent".to_owned()]));
  assert_eq!(
    answered(SENDS_PATH, 200, &json!({ "lines": ["one went"], "refusal": "the other did not" })),
    Err(Unsent::Refused {
      lines: vec!["one went".to_owned()],
      refusal: "the other did not".to_owned(),
    },),
  );
  assert_eq!(
    answered(CONSULT_PATH, 409, &json!({ "error": "nobody answers there" })),
    Err(Unsent::Refused {
      lines: Vec::new(),
      refusal: "nobody answers there".to_owned(),
    },),
  );
  assert_eq!(answered(SENDS_PATH, 502, &Value::Null), Err(Unsent::Untaken(502)));
  assert!(matches!(answered(SENDS_PATH, 200, &json!({})), Err(Unsent::Unreached(_))));
}

#[tokio::test]
async fn a_consult_line_outside_any_agents_turn_is_refused_before_the_receiver_is_asked() {
  let input = ConsultInput {
    text: "Shall we split it?".to_owned(),
    reply: None,
  };

  assert!(matches!(
    ask_consult(&Service::default(), None, &input).await,
    Err(Unsent::Refused { .. })
  ));
}

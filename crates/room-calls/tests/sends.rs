//! A send asked of the running receiver: how it is written, who asks it, and what the receiver's answer says
//! of it (docs/chat-mcp.md, 「How a call runs」).

use room_calls::bot::Service;
use room_calls::sends::{
  Act, Asked, Asker, CONSULT_PATH, ConsultInput, RandomDrawInput, SENDS_PATH, StickerInput, Unsent,
  answered, ask_consult,
};
use room_calls::turn::{Room, Turn};
use serde_json::{Value, json};

const ROOM: i64 = 4_200;

#[test]
fn an_asked_send_is_written_with_its_tool_its_input_and_its_asker() {
  let asked = Asked {
    asker: Asker {
      chat: Some(ROOM),
      agent: Some("agent-a".to_owned()),
      by: Some(format!("chat:{ROOM}")),
      ..Asker::default()
    },
    act: Act::Sticker(StickerInput {
      chat: None,
      via: None,
      emoji: ":+1:".to_owned(),
    }),
  };
  let written = serde_json::to_value(&asked).unwrap();

  assert_eq!(
    written,
    json!({
      "asker": { "chat": ROOM, "agent": "agent-a", "by": format!("chat:{ROOM}") },
      "tool": "sticker",
      "input": { "emoji": ":+1:" },
    }),
  );
  assert_eq!(serde_json::from_value::<Asked>(written).unwrap(), asked);
}

#[test]
fn a_draws_own_fields_leave_out_the_room_the_road_and_the_mode() {
  let input: RandomDrawInput = serde_json::from_value(json!({
    "chat": ROOM,
    "via": "road-a",
    "mode": "sync",
    "shape": "range",
    "min": 1,
    "max": 6,
  }))
  .unwrap();

  assert_eq!(Value::Object(input.request()), json!({ "shape": "range", "min": 1, "max": 6 }));
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
      chat: Some(-200),
      via: None,
      agent: Some("agent-a".to_owned()),
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

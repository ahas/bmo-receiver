//! A send asked of the running receiver: how it is written, who asks it, and what the receiver's answer says
//! of it (docs/chat-mcp.md, 「How a call runs」).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use room_calls::bot::Service;
use room_calls::sends::{
  Act, Asked, Asker, FileInput, FileToSend, PhotoInput, Picture, RandomDrawInput, SENDS_PATH, SayInput,
  SpeakInput, StickerInput, Unsent, VoiceInput, answered, ask_within,
};
use room_calls::turn::{Room, Turn};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

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
fn a_line_a_spoken_talk_a_voice_note_and_a_caption_hold_words_and_nothing_else_does() {
  let photo = |caption: Option<&str>| {
    Act::Photo(PhotoInput {
      chat: None,
      via: None,
      pictures: vec![Picture {
        url: "https://example.com/a.png".to_owned(),
        label: None,
      }],
      caption: caption.map(str::to_owned),
      reply_to: None,
    })
  };
  let say = Act::Say(SayInput {
    chat: None,
    via: None,
    text: "Done.".to_owned(),
    reply: false,
    reply_to: None,
  });
  let sticker = Act::Sticker(StickerInput {
    chat: None,
    via: None,
    emoji: "👍".to_owned(),
    reply_to: None,
  });

  assert!(say.holds_words());
  assert!(photo(Some("Here it is.")).holds_words());
  assert!(!photo(None).holds_words());
  assert!(!photo(Some("  ")).holds_words());
  assert!(!sticker.holds_words());
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
    began: Some(1_000),
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
      began: Some(1_000),
      by: Some(format!("chat:{ROOM}")),
      job: true,
      from: None,
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
    answered(SENDS_PATH, 409, &json!({ "error": "nobody answers there" })),
    Err(Unsent::Refused {
      lines: Vec::new(),
      refusal: "nobody answers there".to_owned(),
    },),
  );
  assert_eq!(answered(SENDS_PATH, 502, &Value::Null), Err(Unsent::Untaken(502)));
  assert!(matches!(answered(SENDS_PATH, 200, &json!({})), Err(Unsent::Unreached(_))));
}

#[test]
fn a_spoken_talk_is_asked_by_its_words_under_the_speak_act() {
  let asked = Asked {
    asker: Asker {
      room: Some(ROOM),
      from: Some("bmo talk".to_owned()),
      ..Asker::default()
    },
    act: Act::Speak(SpeakInput {
      chat: None,
      via: None,
      voice: "voice-a".to_owned(),
      text: "Written words.".to_owned(),
      spoken: Some("Spoken words.".to_owned()),
      reply_to: None,
    }),
  };

  assert_eq!(asked.act.tool(), "speak");
  assert_eq!(
    serde_json::to_value(&asked).unwrap(),
    json!({
      "asker": { "room": ROOM, "from": "bmo talk" },
      "tool": "speak",
      "input": { "voice": "voice-a", "text": "Written words.", "spoken": "Spoken words." },
    }),
  );
}

#[tokio::test]
async fn a_line_of_several_lines_asked_of_a_receiver_that_does_not_answer_ends_within_its_bound() {
  // A receiver that takes every request whole and never answers it.
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let heard = Arc::new(Mutex::new(Vec::<u8>::new()));
  let held = heard.clone();
  let silent = tokio::spawn(async move {
    let (mut connection, _) = listener.accept().await.unwrap();
    let mut read = vec![0_u8; 4096];

    // Everything the call writes is read, and the connection is held until the call lets it go.
    while let Ok(count) = connection.read(&mut read).await {
      if count == 0 {
        break;
      }

      held.lock().unwrap().extend_from_slice(&read[..count]);
    }
  });
  let receiver = Service {
    address,
    token: Some("the-token".to_owned()),
    ..Service::default()
  };
  let text = (1..=9).map(|line| format!("line {line} of the words")).collect::<Vec<_>>().join("\n");
  let asked = Asked {
    asker: Asker {
      room: Some(ROOM),
      via: Some("road-a".to_owned()),
      from: Some("bmo room say".to_owned()),
      ..Asker::default()
    },
    act: Act::Say(SayInput {
      chat: None,
      via: None,
      text,
      reply: false,
      reply_to: None,
    }),
  };

  let ended = tokio::time::timeout(
    Duration::from_secs(10),
    ask_within(&receiver, &asked, Duration::from_millis(200)),
  )
  .await
  .expect("the call ends by its own bound");

  // The call has let its connection go, so the receiver has read the whole of what it wrote.
  tokio::time::timeout(Duration::from_secs(10), silent)
    .await
    .expect("the call let its connection go")
    .unwrap();

  let Err(Unsent::Unreached(why)) = ended else {
    panic!("the call is refused as unanswered: {ended:?}")
  };

  assert!(why.contains("did not answer /sends within 1 seconds"), "{why}");
  assert!(String::from_utf8_lossy(&heard.lock().unwrap()).contains("line 9 of the words"));
}

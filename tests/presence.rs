//! The present held in memory (docs/receiver.md, 「Typing and presence」): a mark ends by its life unless told again,
//! is told off once, and a chat service that stops being ready ends what it told; on a paused clock, so no test waits
//! out a mark's life.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bmo_chat_client::{ChatPresence, ChatPresenceState};
use bmo_receiver::presence::{Mark, Present, Source, Who};
use bmo_receiver::receiver::News;
use serde_json::Value;

// The news a test reads back: every event of the present, in order.
#[derive(Default)]
struct Told(Mutex<Vec<Value>>);

impl Told {
  fn of(&self, kind: &str) -> Vec<Value> {
    self.0.lock().unwrap().iter().filter(|event| event["type"] == kind).cloned().collect()
  }
}

impl News for Told {
  fn message(&self, _chat_id: i64, _message_id: i64) {}

  fn tell_present(&self, _key: &str, event: &Value) {
    self.0.lock().unwrap().push(event.clone());
  }

  fn end_present(&self, _key: &str, event: &Value) {
    self.0.lock().unwrap().push(event.clone());
  }
}

const ROOM: i64 = -5;

fn person() -> Who {
  Who::Person {
    via: "road-a".to_owned(),
    person: 7,
  }
}

fn mark(on: bool) -> Mark {
  Mark {
    room: ROOM,
    who: person(),
    via: Some("road-a".to_owned()),
    on,
    lasts: Some(Duration::from_secs(6)),
    speaker: None,
  }
}

// The paused clock moved on, and every task it woke let run.
async fn after(span: Duration) {
  tokio::time::advance(span).await;

  for _ in 0..8 {
    tokio::task::yield_now().await;
  }
}

#[tokio::test(start_paused = true)]
async fn a_mark_ends_by_its_life_and_a_renewal_stands_for_a_life_from_then() {
  let told = Arc::new(Told::default());
  let present = Present::new(Duration::ZERO, Duration::from_secs(15));

  present.mark(told.clone(), mark(true));
  after(Duration::from_secs(4)).await;
  present.mark(told.clone(), mark(true));
  after(Duration::from_secs(4)).await;

  assert_eq!(present.writing_in(ROOM), [person()], "the renewal outlives the first telling's life");

  after(Duration::from_secs(3)).await;

  let ons: Vec<bool> = told.of("typing").iter().map(|event| event["on"] == true).collect();

  assert!(present.writing_in(ROOM).is_empty());
  assert_eq!(ons, [true, true, false], "told off once, when the renewal's life ran out");

  present.mark(told.clone(), mark(false));

  assert_eq!(told.of("typing").len(), 3, "an off for a mark that no longer stands tells nothing");
}

#[test]
fn a_service_not_ready_ends_its_marks_and_leaves_what_it_told_unknown_while_another_source_stands() {
  let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
  let _inside = runtime.enter();
  let told = Arc::new(Told::default());
  let present = Present::new(Duration::ZERO, Duration::from_secs(15));
  let account = Who::Account(3);
  let seen = |state, at| ChatPresence {
    state,
    last_seen_at: Some(at),
  };
  let provider = Source::Provider {
    via: "road-a".to_owned(),
    person: 9,
  };

  present.mark(
    told.clone(),
    Mark {
      who: account.clone(),
      ..mark(true)
    },
  );
  present.told(
    told.as_ref(),
    Source::Console(3),
    account.clone(),
    Some(seen(ChatPresenceState::Away, 50)),
    Some(vec![ROOM]),
  );
  present.told(
    told.as_ref(),
    provider,
    account.clone(),
    Some(seen(ChatPresenceState::Online, 20)),
    Some(vec![ROOM]),
  );

  assert!(present.watch("road-a") && !present.watch("road-a"), "one look at a service at a time");

  present.not_ready(told.as_ref(), "road-a");

  let states: Vec<(Value, Value)> = told
    .of("presence")
    .iter()
    .map(|event| (event["state"].clone(), event["lastSeenAt"].clone()))
    .collect();

  assert_eq!(told.of("typing").last().unwrap()["on"], false);
  assert_eq!(
    states,
    [("away".into(), 50.into()), ("online".into(), 50.into()), ("away".into(), 50.into())],
    "the console's word stands once the service's is taken back"
  );
  assert!(present.watch("road-a"), "the look is let go with what the service told");
}

//! A spoken `talk` (docs/boundary-api.md, 「The unit command」): the synthesis followed until it is done however
//! long it waits for its card, each `waitingOn` sentence told once as it changes, and one that failed refused.
//! The voice engine is a stand-in on a port of its own, and the clock is paused, so minutes pass at once.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
  Json, Router,
  extract::State,
  http::StatusCode,
  routing::{get, post},
};
use serde_json::{Value, json};
use tokio::time::Instant;

use bmo_receiver::verbs::talk::speak;
use receiver_ports::http::Peer;

/// How long the stand-in's synthesis waits for its card: far past any patience a caller might keep.
const CARD_AFTER: Duration = Duration::from_secs(20 * 60);
/// The sound the stand-in speaks.
const SOUND: &[u8] = b"OggS a spoken line";

/// The voice engine: one synthesis, waiting in the line until `CARD_AFTER` has passed since it was taken in, then
/// spoken, then done or failed as the test sets.
struct StandInVoice {
  taken: Mutex<Option<Instant>>,
  fails: bool,
}

impl StandInVoice {
  // The synthesis as `GET /syntheses/{id}` answers it at this moment.
  fn synthesis(&self) -> Value {
    let waited = self.taken.lock().unwrap().map_or(Duration::ZERO, |taken| taken.elapsed());
    let (state, waiting_on) = if waited < CARD_AFTER / 2 {
      ("waiting", Some("Every card is in use."))
    } else if waited < CARD_AFTER {
      ("waiting", Some("Second in the line for a card."))
    } else if waited < CARD_AFTER + Duration::from_secs(5) {
      ("speaking", None)
    } else if self.fails {
      ("failed", None)
    } else {
      ("done", None)
    };

    json!({ "id": 7, "voiceName": "voice-a", "state": state, "waitingOn": waiting_on })
  }
}

async fn synthesise(State(voice): State<Arc<StandInVoice>>) -> (StatusCode, Json<Value>) {
  *voice.taken.lock().unwrap() = Some(Instant::now());

  (StatusCode::CREATED, Json(voice.synthesis()))
}

async fn synthesis(State(voice): State<Arc<StandInVoice>>) -> Json<Value> {
  Json(voice.synthesis())
}

async fn sound() -> Vec<u8> {
  SOUND.to_vec()
}

// The stand-in served on a port of its own, and the address the verb is given.
async fn engine(fails: bool) -> Peer {
  let voice = Arc::new(StandInVoice {
    taken: Mutex::new(None),
    fails,
  });
  let app = Router::new()
    .route("/syntheses", post(synthesise))
    .route("/syntheses/{id}", get(synthesis))
    .route("/syntheses/{id}/sound", get(sound))
    .with_state(voice);
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  Peer {
    base,
    token: String::new(),
  }
}

#[tokio::test(start_paused = true)]
async fn a_synthesis_waiting_long_for_its_card_is_followed_until_it_is_done() {
  let engine = engine(false).await;
  let started = Instant::now();
  let mut told = Vec::new();

  let spoken = speak(&engine, "voice-a", "A line to say.", |id, waiting_on| {
    told.push(format!("{id}: {waiting_on}"))
  })
  .await
  .unwrap();

  assert_eq!(spoken, SOUND, "the sound is the one the engine spoke");
  assert!(started.elapsed() >= CARD_AFTER, "the wait ran as long as the card took");
  assert_eq!(
    told,
    ["7: Every card is in use.", "7: Second in the line for a card."],
    "each sentence the synthesis waits on is told once, as it changes"
  );
}

#[tokio::test(start_paused = true)]
async fn a_synthesis_that_failed_after_its_wait_is_refused() {
  let engine = engine(true).await;

  let refused = speak(&engine, "voice-a", "A line to say.", |_, _| ()).await.unwrap_err();

  assert_eq!(refused.to_string(), "talk: synthesis 7 ended failed");
}

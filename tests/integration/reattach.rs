//! A turn is a resource the round re-attaches to, never one held call (docs/bmo-window.md, 「Quiet instruction」):
//! a stream the agency cut mid-turn is opened again, the end that came meanwhile read off the events kept or off
//! the turn itself, a turn the agency holds no record of fails uncounted, a turn never named asks to be sent
//! again under its key, and a look that carried no words probes the named turn where the stream stays silent.

use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::attach::{Followed, Probed, Step};
use bmo_receiver::chat::conversation::Failure;
use receiver_ports::agency::{Delivery, Event, TurnRead};
use serde_json::json;

use crate::integration::common::units::StandInAgency;

const SESSION: &str = "agent-a@agent";
const KINDS: &[&str] = &["turn.created", "turn.ended"];

fn event(id: i64, kind: &str, turn: i64) -> Event {
  Event {
    kind: kind.to_owned(),
    data: json!({ "session": SESSION, "turnId": turn, "run": "run-1", "outcome": "answered", "answer": "kept" }),
    id: Some(id),
    at: Some(id),
  }
}

// A turn named on a stream that the agency then cut, followed from the event before it; what the agency answers
// after the cut is set before it.
async fn cut_after_its_start(agency: &Arc<StandInAgency>) -> Followed {
  agency.streaming(vec![vec![event(10, "turn.created", 5)], vec![]]);

  let mut followed =
    Followed::open(agency.clone(), SESSION, KINDS, Some(9), Duration::from_millis(1)).await;

  assert!(matches!(followed.next().await, Ok(Step::Event(_))));
  assert_eq!((followed.turn(), followed.run().as_deref()), (Some(5), Some("run-1")));

  followed
}

#[tokio::test]
async fn a_cut_turn_is_re_attached_to_and_its_end_read_off_the_turn() {
  let agency = Arc::new(StandInAgency::new());

  agency.reading(
    5,
    TurnRead {
      state: "ended".to_owned(),
      outcome: Some("answered".to_owned()),
      answer: Some("read".to_owned()),
      reason: None,
      delivery: Some(Delivery::Delivered),
    },
  );

  let mut followed = cut_after_its_start(&agency).await;

  let Ok(Step::Ended(end)) = followed.next().await else {
    panic!("the turn's end is read off the turn")
  };

  assert_eq!((end["answer"].as_str(), end["delivery"].as_str()), (Some("read"), Some("delivered")));
}

#[tokio::test]
async fn an_end_that_came_while_the_stream_was_cut_is_read_off_the_events_kept() {
  let agency = Arc::new(StandInAgency::new());

  agency.keeping(vec![event(10, "turn.created", 5), event(11, "turn.ended", 5)]);

  let mut followed = cut_after_its_start(&agency).await;

  let Ok(Step::Ended(end)) = followed.next().await else {
    panic!("the kept end is the turn's")
  };

  assert_eq!(end["answer"].as_str(), Some("kept"));
}

#[tokio::test]
async fn a_turn_the_agency_holds_no_record_of_fails_uncounted() {
  let agency = Arc::new(StandInAgency::new());
  let mut followed = cut_after_its_start(&agency).await;

  let failure = followed.next().await.unwrap_err();

  assert!(matches!(failure, Failure::Transport(_)), "{failure:?}");
  assert!(!failure.counted(), "a turn the agency lost counts against nothing");
}

#[tokio::test]
async fn a_turn_never_named_asks_to_be_sent_again() {
  let agency = Arc::new(StandInAgency::new());

  agency.streaming(vec![vec![], vec![]]);

  let mut followed =
    Followed::open(agency.clone(), SESSION, KINDS, Some(9), Duration::from_millis(1)).await;

  assert!(matches!(followed.next().await, Ok(Step::Unnamed)));
  assert_eq!(followed.turn(), None);
}

#[tokio::test]
async fn a_turn_read_as_ended_with_its_end_nowhere_fails_uncounted_rather_than_answer_nothing() {
  let agency = Arc::new(StandInAgency::new());

  agency.reading(
    5,
    TurnRead {
      state: "ended".to_owned(),
      ..TurnRead::default()
    },
  );

  let mut followed = cut_after_its_start(&agency).await;

  let failure = followed.next().await.unwrap_err();

  assert!(matches!(failure, Failure::Transport(_)), "{failure:?}");
  assert!(!failure.counted());
}

#[tokio::test]
async fn an_end_the_agency_names_under_turn_is_the_end_of_that_turn_alone() {
  let agency = Arc::new(StandInAgency::new());
  let end = |id: i64, turn: i64, answer: &str| Event {
    kind: "turn.ended".to_owned(),
    data: json!({ "session": SESSION, "turn": turn, "outcome": "answered", "answer": answer, "delivery": "taken" }),
    id: Some(id),
    at: Some(id),
  };

  agency.keeping(vec![end(11, 4, "an earlier turn's"), end(12, 5, "its own")]);

  let mut followed = cut_after_its_start(&agency).await;

  let Ok(Step::Ended(ended)) = followed.next().await else {
    panic!("the kept end is the turn's")
  };

  assert_eq!(ended["answer"].as_str(), Some("its own"));
}

#[tokio::test]
async fn a_probe_of_the_named_turn_carries_its_end_or_tells_it_gone_or_says_nothing() {
  let running = Arc::new(StandInAgency::new());
  running.reading(
    5,
    TurnRead {
      state: "running".to_owned(),
      ..TurnRead::default()
    },
  );
  let followed = cut_after_its_start(&running).await;

  assert!(matches!(followed.probe().await, Probed::Going), "a turn still working is followed on");

  let ended = Arc::new(StandInAgency::new());
  ended.reading(
    5,
    TurnRead {
      state: "ended".to_owned(),
      outcome: Some("answered".to_owned()),
      answer: Some("read".to_owned()),
      reason: None,
      delivery: Some(Delivery::Delivered),
    },
  );
  let followed = cut_after_its_start(&ended).await;

  let Probed::Ended(end) = followed.probe().await else {
    panic!("a read carrying the end gives it as the stream's own end would")
  };

  assert_eq!((end["answer"].as_str(), end["delivery"].as_str()), (Some("read"), Some("delivered")));

  let unreadable = Arc::new(StandInAgency::new());
  unreadable.reading(
    5,
    TurnRead {
      state: "ended".to_owned(),
      ..TurnRead::default()
    },
  );
  let followed = cut_after_its_start(&unreadable).await;

  assert!(
    matches!(followed.probe().await, Probed::Gone),
    "an end the read does not carry is unheard"
  );

  let unrecorded = Arc::new(StandInAgency::new());
  let followed = cut_after_its_start(&unrecorded).await;

  assert!(
    matches!(followed.probe().await, Probed::Gone),
    "no record of the turn is an end gone past reading"
  );

  let absent = Arc::new(StandInAgency::new());
  absent.turn_read_unreachable.store(true, std::sync::atomic::Ordering::SeqCst);
  let followed = cut_after_its_start(&absent).await;

  assert!(
    matches!(followed.probe().await, Probed::Going),
    "an agency that answers nothing says nothing"
  );
}

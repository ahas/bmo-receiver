//! Words pushed into a working turn (docs/boundary-api.md, 「The agency」; docs/bmo-window.md, `push`): what the
//! round offers goes to the agency's session push under the turn's session, the round settles on what the agency
//! said, and a refusal or a push that failed holds the next offer for the policy's gap.

use std::sync::Mutex;
use std::time::Duration;

use bmo_receiver::chat::ask::pushed;
use bmo_receiver::chat::conversation::{Aside, AsideOutcome, BoxFuture};
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_ports::agency::Said;

use crate::common::units::StandInAgency;

const SESSION: &str = "agent-a@-7";
const TURN: i64 = 7;

// A round offering its words once, and keeping what it was told became of them.
struct Offering {
  words: Mutex<Option<String>>,
  settled: Mutex<Vec<AsideOutcome>>,
}

impl Offering {
  fn of(words: Option<&str>) -> Self {
    Offering {
      words: Mutex::new(words.map(str::to_owned)),
      settled: Mutex::default(),
    }
  }

  fn settled(&self) -> Vec<AsideOutcome> {
    self.settled.lock().unwrap().clone()
  }
}

impl Aside for Offering {
  fn offer(&self) -> BoxFuture<'_, Option<String>> {
    let words = self.words.lock().unwrap().take();

    Box::pin(async move { words })
  }

  fn settle(&self, outcome: AsideOutcome) -> BoxFuture<'_, ()> {
    self.settled.lock().unwrap().push(outcome);

    Box::pin(async {})
  }
}

// An agency saying `said` of every push, or failing every push where it says nothing.
fn saying(said: Option<Said>) -> StandInAgency {
  let agency = StandInAgency::new();

  *agency.saying.lock().unwrap() = said;
  agency
}

#[tokio::test]
async fn words_the_turn_took_settle_as_delivered_and_hold_no_gap() {
  let agency = saying(Some(Said::Delivered));
  let round = Offering::of(Some("more words"));

  assert_eq!(pushed(&agency, SESSION, TURN, &round).await, None);
  assert_eq!(*agency.pushes.lock().unwrap(), [(SESSION.to_owned(), TURN, "more words".to_owned())]);
  assert_eq!(round.settled(), [AsideOutcome::Delivered]);
}

#[tokio::test]
async fn a_turn_waiting_on_a_person_settles_as_waiting_and_holds_no_gap() {
  let agency = saying(Some(Said::Waiting));
  let round = Offering::of(Some("more words"));

  assert_eq!(pushed(&agency, SESSION, TURN, &round).await, None);
  assert_eq!(round.settled(), [AsideOutcome::Waiting]);
}

#[tokio::test]
async fn a_refused_push_and_a_failed_one_settle_as_refused_and_hold_the_next_offer_for_the_gap() {
  let gap = Duration::from_millis(CHAT_ROUND.pushing.refused_gap_ms);

  for said in [Some(Said::Refused), None] {
    let agency = saying(said);
    let round = Offering::of(Some("more words"));
    let before = tokio::time::Instant::now();
    let until = pushed(&agency, SESSION, TURN, &round).await.expect("a refusal holds a gap");

    assert!(until >= before + gap, "the next offer waits the policy's gap");
    assert_eq!(round.settled(), [AsideOutcome::Refused]);
  }
}

#[tokio::test]
async fn a_round_with_nothing_to_offer_pushes_nothing() {
  let agency = saying(Some(Said::Delivered));
  let round = Offering::of(None);

  assert_eq!(pushed(&agency, SESSION, TURN, &round).await, None);
  assert!(agency.pushes.lock().unwrap().is_empty());
  assert!(round.settled().is_empty());
}

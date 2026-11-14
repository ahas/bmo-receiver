//! A chat service's question about a room answered off what the receiver keeps (docs/receiver.md, 「What is kept for
//! a moment」): a service that asks the receiver back while it answers the receiver's own question is asked once,
//! however often the room's facts are read.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::integration::common::Scratch;
use crate::integration::common::queue::stood_on;
use crate::integration::common::units::{OWNER, StandInChat};

#[tokio::test]
async fn a_service_asking_back_while_it_answers_is_asked_once_for_the_room() {
  let scratch = Scratch::new().await;
  let service = Arc::new(StandInChat::new("road-a").holding(&[-500]).titling(-500, "Asked Room"));
  let front = stood_on(&scratch, OWNER, vec![service.clone()]).await;
  let receiver = front.receiver();

  service.asks_back(Arc::new(receiver.clone()));

  // The room list reads every room's facts on each pass; each read after the first reads the kept answer.
  for _ in 0..20 {
    assert!(!receiver.roads().is_owner_room(-500).await);
  }

  assert_eq!(receiver.roads().room_road(-500).await.as_deref(), Some("road-a"));
  assert_eq!(
    service.rooms_asked.load(Ordering::SeqCst),
    1,
    "the receiver's answer to the service's question asks the service nothing"
  );
}

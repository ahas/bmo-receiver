//! What the running receiver keeps by room, forgotten as a room's registration is removed (docs/receiver.md,
//! 「Removing a room's registration」): each holder of live per-room state declares, beside its own code, how it
//! forgets one room. A forgetting writes nothing to the database, whose rows the removal takes in its own write.

use std::pin::Pin;

use crate::receiver::Receiver;

/// What a forgetting comes to, which it may take its time over.
pub type Forgetting<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// One holder of live per-room state, and how it forgets a room.
pub struct RoomForget {
  pub what: &'static str,
  pub forget: for<'a> fn(&'a Receiver, i64) -> Forgetting<'a>,
}

inventory::collect!(RoomForget);

/// Declares how a holder of live per-room state forgets a room, beside the code that holds it.
#[macro_export]
macro_rules! room_forget {
  ($forget:expr) => {
    inventory::submit! { $forget }
  };
}

/// Every holder declared in this binary.
pub fn holders() -> impl Iterator<Item = &'static RoomForget> {
  inventory::iter::<RoomForget>.into_iter()
}

/// Has every holder forget the room.
pub async fn forget(receiver: &Receiver, room: i64) {
  for holder in holders() {
    (holder.forget)(receiver, room).await;
  }
}

//! A login's end, as the agency's logins' watch tells it (docs/voiced-notices.md, 「A login's end」): the notice
//! its policy composes, with the one command the owner runs, said in the owner's own chat as it stands.

use futures_util::future::BoxFuture;
use receiver_ports::agency::Event;
use serde_json::Value;

use crate::hearing::Hearer;
use crate::receiver::Receiver;

/// The kind a login's end is published under.
pub const LOGIN_NOTICE: &str = "account.changed";

/// A login's end as the service hears it on the agency's stream.
pub struct Logins {
  receiver: Receiver,
}

impl Logins {
  pub fn new(receiver: Receiver) -> Self {
    Logins { receiver }
  }
}

impl Hearer for Logins {
  fn kinds(&self) -> &'static [&'static str] {
    &[LOGIN_NOTICE]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      let Some(notice) =
        event.data.get("notice").and_then(Value::as_str).filter(|said| !said.trim().is_empty())
      else {
        eprintln!("bmo-receiver: a login's end named no notice");
        return;
      };

      if let Err(why) = self.receiver.speech().say_to_owner(notice.trim()).await {
        eprintln!("bmo-receiver: a login's end was not said in the owner's chat: {why}");
      }
    })
  }
}

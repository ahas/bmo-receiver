//! What the present is told from (docs/receiver.md, 「Typing and presence」): a chat service's typing marks and
//! presence changes, the console's account presence and typing, a message kept in a room, and the room's answerer
//! shown typing; each read as who it is of and the rooms they stand in, then held and told by `Present`.

use std::time::Duration;

use bmo_chat_client::{ChatPresence, PresenceChange, TYPING_MARK_LASTS_MS, TypingMark};
use serde_json::Value;

use super::{Mark, Source, Who};
use crate::accounts::{self, Speaker};
use crate::receiver::{Inbound, Receiver, reads};
use crate::roads::providers::{self, SaidBy};
use crate::{Fault, all};

/// The account a person is linked to, as the web unit links a sender to a user, kept for the policy's span; a link
/// that cannot be read is none, and kept not at all, so the next asks again.
pub async fn linked(receiver: &Receiver, person: i64) -> Option<i64> {
  let present = receiver.present();

  if let Some(account) = present.link(person) {
    return account;
  }

  let user = receiver.units().web.user_of_sender(person).await.ok()?;
  let account = user.and_then(|user| user.trim().parse::<i64>().ok());

  present.keep_link(person, account);

  account
}

/// Who a chat service's person is: the account they are linked to, else the service's own person.
pub async fn who_of(receiver: &Receiver, via: &str, person: i64) -> Who {
  match linked(receiver, person).await {
    Some(account) => Who::Account(account),
    None => Who::Person {
      via: via.to_owned(),
      person,
    },
  }
}

/// The rooms the receiver holds that someone stands in: for a person, each room holding a message of theirs; for an
/// account, each room it is a member of and each holding a message of a sender linked to it.
pub async fn rooms_of(receiver: &Receiver, who: &Who) -> Result<Vec<i64>, Fault> {
  let db = receiver.db();
  let (mut rooms, senders) = match who {
    Who::Person { person, .. } => (Vec::new(), vec![*person]),
    Who::Account(account) => {
      let mut senders =
        receiver.units().web.senders_of_user(&account.to_string()).await.unwrap_or_default();

      senders.extend(accounts::identity_of(db, *account).await?);

      (accounts::rooms_of(db, *account).await?, senders)
    }
    Who::Answerer(_) => return Ok(Vec::new()),
  };

  for sender in senders {
    let spoken = all(
      db,
      r#"SELECT DISTINCT "chat_id" FROM "messages" WHERE "sender_id" = $1"#,
      vec![sender.into()],
    )
    .await?;

    for row in spoken {
      rooms.push(row.try_get::<i64>("", "chat_id")?);
    }
  }

  rooms.sort_unstable();
  rooms.dedup();

  Ok(rooms)
}

/// A chat service's typing mark held and told, where the receiver holds the room and the room takes what the service
/// carries; any other passed over.
pub async fn typing_marked(receiver: &Receiver, mark: TypingMark) {
  if !takes(receiver, mark.room_id, &mark.via).await {
    return;
  }

  let who = who_of(receiver, &mark.via, mark.person_id).await;

  receiver.present().mark(
    receiver.news_shared(),
    Mark {
      room: mark.room_id,
      who,
      via: Some(mark.via.clone()),
      on: mark.on,
      lasts: Some(mark.lasts()),
      speaker: None,
    },
  );

  if mark.on {
    watch(receiver, &mark.via);
  }
}

// Whether a room takes what a chat service tells of it: one the receiver holds whose chosen list, where it chose
// one, names the service.
async fn takes(receiver: &Receiver, room: i64, via: &str) -> bool {
  let held = room == receiver.owner_room() || reads::recorded(receiver.db(), room).await.unwrap_or(false);

  held && receiver.roads().admits(room, via).await
}

/// A chat service's presence change held and told: each person listed as what the service says of them, passed
/// over where they stand in no room the receiver holds; with `whole`, every person of the service it does not list
/// taken back.
pub async fn presence_changed(receiver: &Receiver, change: PresenceChange) {
  let present = receiver.present();
  let news = receiver.news();
  let via = change.via;

  for person in &change.people {
    let who = who_of(receiver, &via, person.person_id).await;
    let rooms = match rooms_of(receiver, &who).await {
      Ok(rooms) => rooms,
      Err(why) => {
        eprintln!("presence: the rooms of {} could not be read: {}", person.person_id, why.detail);

        continue;
      }
    };
    let presence = (!rooms.is_empty()).then_some(person.presence);
    let source = Source::Provider {
      via: via.clone(),
      person: person.person_id,
    };

    present.told(news, source, who, presence, Some(rooms));
  }

  if change.whole {
    for (person, who) in present.told_by(&via) {
      if !change.people.iter().any(|listed| listed.person_id == person) {
        present.told(
          news,
          Source::Provider {
            via: via.clone(),
            person,
          },
          who,
          None,
          None,
        );
      }
    }
  }

  watch(receiver, &via);
}

/// The console's word on an account's presence, one source among the account's; none takes it back.
pub async fn console_presence(
  receiver: &Receiver,
  account: i64,
  presence: Option<ChatPresence>,
) -> Result<(), Fault> {
  let who = Who::Account(account);
  let rooms = rooms_of(receiver, &who).await?;

  receiver
    .present()
    .told(receiver.news(), Source::Console(account), who, presence, Some(rooms));

  Ok(())
}

/// An account's typing in a room from the console: held for the base's life and told, then handed to the room's
/// chat services in the writer's own voice.
pub async fn console_typing(receiver: &Receiver, room: i64, account: i64, on: bool) -> Result<(), Fault> {
  receiver.present().mark(
    receiver.news_shared(),
    Mark {
      room,
      who: Who::Account(account),
      via: None,
      on,
      lasts: Some(Duration::from_millis(TYPING_MARK_LASTS_MS)),
      speaker: None,
    },
  );

  let said_by = match accounts::speaker(receiver, account).await {
    Ok(Speaker::Owner) => SaidBy::Owner,
    Ok(Speaker::Account { .. }) => SaidBy::Account {
      account,
      name: String::new(),
    },
    Err(why) => {
      eprintln!("chat {room}: account {account}'s typing goes to no chat service: {why}");

      return Ok(());
    }
  };
  let receiver = receiver.clone();

  // The console renews a mark every few seconds, so its route answers at once and the sends go on behind it.
  tokio::spawn(async move {
    providers::typing(receiver.db(), receiver.roads(), room, &said_by, on).await;
  });

  Ok(())
}

/// A message kept live in a room ends the mark of the one who said it there.
pub async fn spoke(receiver: &Receiver, inbound: &Inbound) {
  let Some(sender) = inbound.sender_id else { return };
  let present = receiver.present();
  let writing = present.writing_in(inbound.chat_id);

  if writing.is_empty() {
    return;
  }

  let said = who_of(receiver, &inbound.via, sender).await;

  for who in writing.iter().filter(|who| who.spoke(sender, &said)) {
    present.spoke(receiver.news(), inbound.chat_id, who);
  }
}

/// The room's answerer shown typing on its chat services, or no longer, told under the speaker its turns are told
/// under; its mark stands until it is told off.
pub fn answerer(receiver: &Receiver, room: i64, agent: &str, speaker: Option<Value>, on: bool) {
  receiver.present().mark(
    receiver.news_shared(),
    Mark {
      room,
      who: Who::Answerer(agent.to_owned()),
      via: None,
      on,
      lasts: None,
      speaker,
    },
  );
}

// The chat service's readiness looked at while anything it told stands: a look that finds it not ready, or not
// answering, ends what it told.
fn watch(receiver: &Receiver, via: &str) {
  let present = receiver.present();

  if !present.watch(via) {
    return;
  }

  let receiver = receiver.clone();
  let via = via.to_owned();

  tokio::spawn(async move {
    let present = receiver.present();

    loop {
      tokio::time::sleep(present.look()).await;

      if !present.still_watched(&via) {
        return;
      }

      let ready = match receiver.roads().chats().road(&via) {
        Some(service) => service.health().await.is_ok_and(|health| health.ready),
        None => false,
      };

      if !ready {
        present.not_ready(receiver.news(), &via);

        return;
      }
    }
  });
}

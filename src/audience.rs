//! Whether someone outside the company sits in a room (docs/receiver.md, 「Whether someone outside the company sits
//! in a room」): a room's audience for a supervisor, `internal` where everyone in it is read and stands in the
//! company of the supervisor's owner, and `external` in every other case, a failure to read among them.

use std::fmt::Display;

use receiver_ports::gate::Audience;

use crate::accounts::{self, Privacy};
use crate::receiver::Receiver;
use crate::receiver::reads::agent_numbers;
use crate::receiver::wake::agents;
use crate::roads::providers;

/// What could not be read on the way to a room's audience, which reads it external.
struct Unread(String);

impl<T: Display> From<T> for Unread {
  fn from(why: T) -> Self {
    Self(why.to_string())
  }
}

/// Who sits in a room: accounts, each its own user, and the people its chat services list, each by its sender
/// number; none where someone sits there whom the receiver cannot name.
struct Seated {
  accounts: Vec<i64>,
  people: Vec<i64>,
}

/// A room's audience for a supervisor, read afresh. A failure to read who sits there or where they stand reads
/// external, and the journal says why.
pub async fn audience(receiver: &Receiver, room: i64, agent: &str) -> Audience {
  match read(receiver, room, agent).await {
    Ok(audience) => audience,
    Err(Unread(why)) => {
      println!(
        "chat {room}: the room reads external for {agent}, since who sits there was not read: {why}"
      );
      Audience::External
    }
  }
}

async fn read(receiver: &Receiver, room: i64, agent: &str) -> Result<Audience, Unread> {
  let Some(company) = owners_company(receiver, agent).await? else {
    return Ok(Audience::External);
  };
  let Some(seated) = seated(receiver, room).await? else {
    return Ok(Audience::External);
  };

  for account in seated.accounts {
    if company_of(receiver, &account.to_string()).await? != Some(company) {
      return Ok(Audience::External);
    }
  }

  for person in seated.people {
    let Some(user) = receiver.units().web.user_of_sender(person).await? else {
      return Ok(Audience::External);
    };

    if company_of(receiver, &user).await? != Some(company) {
      return Ok(Audience::External);
    }
  }

  Ok(Audience::Internal)
}

// The company of the account the agency registers as the agent's owner; none for an agent of no account, one the
// agency does not register, and an owner placed in no unit.
async fn owners_company(receiver: &Receiver, agent: &str) -> Result<Option<i64>, Unread> {
  let owner = agents::registered(receiver, agent)
    .await?
    .and_then(|designated| designated.owner_account);

  match owner {
    Some(account) => company_of(receiver, &account.to_string()).await,
    None => Ok(None),
  }
}

async fn company_of(receiver: &Receiver, user: &str) -> Result<Option<i64>, Unread> {
  Ok(receiver.units().web.company_of(user).await?)
}

// A private room seats its one account; a sealed one seats someone not known yet. Any other room seats its members
// by account and everyone each chat service on its list lists in it, less the agents' own numbers.
async fn seated(receiver: &Receiver, room: i64) -> Result<Option<Seated>, Unread> {
  match accounts::privacy(receiver, room).await? {
    Privacy::Of(account) => {
      return Ok(Some(Seated {
        accounts: vec![account],
        people: vec![],
      }));
    }
    Privacy::Sealed => return Ok(None),
    Privacy::Shared => {}
  }

  let db = receiver.db();
  let roads = receiver.roads();
  let accounts = accounts::members(db, room).await?.iter().map(|member| member.account).collect();
  let names: Vec<String> = agents::listed_designated(receiver)
    .await?
    .into_iter()
    .map(|designated| designated.name)
    .collect();
  let own = agent_numbers(db, &names).await?;
  let mut people = vec![];

  // The record is no chat service and lists nobody: whoever reads a room through it is one of its accounts.
  for key in providers::listed(db, roads, room).await?.providers {
    let Some(service) = roads.chats().road(&key) else { continue };

    for member in service.members(room).await?.members {
      let Some(person) = member.person else { return Ok(None) };

      if !own.contains(&person) && !people.contains(&person) {
        people.push(person);
      }
    }
  }

  Ok(Some(Seated { accounts, people }))
}

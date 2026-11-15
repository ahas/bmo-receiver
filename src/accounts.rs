//! The console's accounts as the receiver keeps them (docs/receiver.md, 「Console rooms and accounts」): each
//! account's record identity, a room's members by account, and the private rooms one account alone speaks in. The
//! accounts themselves are the web unit's; the receiver keeps only what its rooms need of them.

use std::collections::BTreeMap;

use sea_orm::DatabaseConnection;

use crate::receiver::wake::agents::{Designated, listed_designated};
use crate::receiver::{NotReceived, Receiver};
use crate::roads::Road;
use crate::roads::record::next_sender;
use crate::store::Fault;
use crate::{all, now_ms, one, run};

/// How often an identity is tried for before the fault stands: two accounts made at once may reach for one number.
const TRIES: usize = 3;

/// The key an account's record identity stands under among the record's numbered people, apart from every agent's
/// name.
pub fn identity_key(account: i64) -> String {
  format!("account:{account}")
}

/// An account's record identity, where it stands.
pub async fn identity_of(db: &DatabaseConnection, account: i64) -> Result<Option<i64>, Fault> {
  one(
    db,
    r#"SELECT "sender_id" FROM "provider_users" WHERE "provider" = $1 AND "user_id" = $2"#,
    vec![Road::Record.key().into(), identity_key(account).into()],
  )
  .await?
  .map(|row| row.try_get::<i64>("", "sender_id"))
  .transpose()
  .map_err(Fault::from)
}

/// An account's record identity, made where it does not stand, and the same number every time after.
pub async fn identity(db: &DatabaseConnection, account: i64) -> Result<i64, Fault> {
  let road = Road::Record.key();
  let mut fault = None;

  for _ in 0..TRIES {
    if let Some(held) = identity_of(db, account).await? {
      return Ok(held);
    }

    let sender = next_sender(db, road).await?;
    let made = run(
      db,
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ($1, $2, $3)
         ON CONFLICT ("provider", "user_id") DO NOTHING"#,
      vec![road.into(), identity_key(account).into(), sender.into()],
    )
    .await;

    if let Err(why) = made {
      fault = Some(why);
    }
  }

  match identity_of(db, account).await? {
    Some(held) => Ok(held),
    None => Err(fault.unwrap_or_else(|| Fault::new(format!("account {account} was given no identity")))),
  }
}

/// A member of a room by account: who let it in and when, both none for a private room's own account, which
/// nobody let in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
  pub account: i64,
  pub added_by: Option<i64>,
  pub added_at: Option<i64>,
}

fn member_of(row: &sea_orm::QueryResult) -> Result<Member, Fault> {
  Ok(Member {
    account: row.try_get("", "account")?,
    added_by: Some(row.try_get("", "added_by")?),
    added_at: Some(row.try_get("", "added_at")?),
  })
}

/// The room's members by account now, in the order they were let in.
pub async fn members(db: &DatabaseConnection, chat_id: i64) -> Result<Vec<Member>, Fault> {
  all(
    db,
    r#"SELECT "account", "added_by", "added_at" FROM "room_accounts"
       WHERE "chat_id" = $1 AND "left_at" IS NULL ORDER BY "added_at", "account""#,
    vec![chat_id.into()],
  )
  .await?
  .iter()
  .map(member_of)
  .collect()
}

/// The account as a member of the room now, where it is one.
pub async fn member(db: &DatabaseConnection, chat_id: i64, account: i64) -> Result<Option<Member>, Fault> {
  one(
    db,
    r#"SELECT "account", "added_by", "added_at" FROM "room_accounts"
       WHERE "chat_id" = $1 AND "account" = $2 AND "left_at" IS NULL"#,
    vec![chat_id.into(), account.into()],
  )
  .await?
  .as_ref()
  .map(member_of)
  .transpose()
}

/// An account let into a room by `by`: the member as it stands, and whether it was let in now rather than being
/// one already.
pub async fn let_in(
  db: &DatabaseConnection,
  chat_id: i64,
  account: i64,
  by: i64,
) -> Result<(Member, bool), Fault> {
  let made = run(
    db,
    r#"INSERT INTO "room_accounts" ("chat_id", "account", "added_by", "added_at") VALUES ($1, $2, $3, $4)
       ON CONFLICT ("chat_id", "account") DO UPDATE
         SET "added_by" = excluded."added_by", "added_at" = excluded."added_at", "left_by" = NULL, "left_at" = NULL
         WHERE "room_accounts"."left_at" IS NOT NULL"#,
    vec![chat_id.into(), account.into(), by.into(), now_ms().into()],
  )
  .await?;
  let standing = member(db, chat_id, account)
    .await?
    .ok_or_else(|| Fault::new(format!("account {account} stands in no room {chat_id} once let in")))?;

  Ok((standing, made > 0))
}

/// An account taken out of a room by `by`, itself where it leaves: whether it was a member until now.
pub async fn take_out(db: &DatabaseConnection, chat_id: i64, account: i64, by: i64) -> Result<bool, Fault> {
  let taken = run(
    db,
    r#"UPDATE "room_accounts" SET "left_by" = $3, "left_at" = $4
       WHERE "chat_id" = $1 AND "account" = $2 AND "left_at" IS NULL"#,
    vec![chat_id.into(), account.into(), by.into(), now_ms().into()],
  )
  .await?;

  Ok(taken > 0)
}

/// The rooms an account is a member of now, by account, in room order.
pub async fn rooms_of(db: &DatabaseConnection, account: i64) -> Result<Vec<i64>, Fault> {
  all(
    db,
    r#"SELECT "chat_id" FROM "room_accounts" WHERE "account" = $1 AND "left_at" IS NULL ORDER BY "chat_id""#,
    vec![account.into()],
  )
  .await?
  .iter()
  .map(|row| row.try_get::<i64>("", "chat_id").map_err(Fault::from))
  .collect()
}

/// The name a room stands under on the record, a supervisor's own room's being the supervisor's name: none for an
/// agent room, which stands under its own number, and for every room not on the record.
pub async fn own_room_name(db: &DatabaseConnection, chat_id: i64) -> Result<Option<String>, Fault> {
  let named = one(
    db,
    r#"SELECT "room_id" FROM "provider_rooms" WHERE "provider" = $1 AND "chat_id" = $2"#,
    vec![Road::Record.key().into(), chat_id.into()],
  )
  .await?
  .map(|row| row.try_get::<String>("", "room_id"))
  .transpose()?;

  Ok(named.filter(|named| *named != chat_id.to_string()))
}

/// The supervisor whose own room on the record a room is, the one written under its name as it was added: none for
/// any other room.
pub async fn supervisor_of_room(
  db: &DatabaseConnection,
  chat_id: i64,
  agents: &[Designated],
) -> Result<Option<Designated>, Fault> {
  let named = own_room_name(db, chat_id).await?;

  Ok(named.and_then(|named| agents.iter().find(|agent| agent.name == named).cloned()))
}

/// The supervisor's own room on the record, where it was given one.
pub async fn room_of_supervisor(db: &DatabaseConnection, name: &str) -> Result<Option<i64>, Fault> {
  one(
    db,
    r#"SELECT "chat_id" FROM "provider_rooms" WHERE "provider" = $1 AND "room_id" = $2"#,
    vec![Road::Record.key().into(), name.into()],
  )
  .await?
  .map(|row| row.try_get::<i64>("", "chat_id"))
  .transpose()
  .map_err(Fault::from)
}

/// The account the web unit links to the owner's sender, where it links one.
pub async fn owner_account(receiver: &Receiver) -> Result<Option<i64>, NotReceived> {
  let user = receiver.units().web.user_of_sender(receiver.owner_id()).await?;

  Ok(user.and_then(|user| user.trim().parse::<i64>().ok()))
}

/// Whose private room a room is (docs/receiver.md, 「Console rooms and accounts」).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Privacy {
  /// No one's: a group room, whoever is left in it.
  Shared,
  /// One account's: the owner's private room to the owner's account, a BMO's own room to its supervisor's owner
  /// account.
  Of(i64),
  /// A private room whose account cannot be named now, so no account writes in it: the owner's private room while
  /// the web unit links no account to the owner's sender.
  Sealed,
}

impl Privacy {
  /// The account it is private to, where one can be named.
  pub fn account(self) -> Option<i64> {
    match self {
      Self::Of(account) => Some(account),
      Self::Shared | Self::Sealed => None,
    }
  }
}

/// Whose private room a room is: the owner's private room the owner's account's, sealed while that cannot be named,
/// a supervisor's own room on the record its owner account's, and no one's for every other room. A unit that does
/// not answer fails the read rather than reading the room as no one's.
pub async fn privacy(receiver: &Receiver, chat_id: i64) -> Result<Privacy, NotReceived> {
  if chat_id == receiver.owner_room() {
    return Ok(owner_account(receiver).await?.map_or(Privacy::Sealed, Privacy::Of));
  }

  let agents = listed_designated(receiver).await?;

  Ok(
    supervisor_of_room(receiver.db(), chat_id, &agents)
      .await?
      .and_then(|agent| agent.owner_account)
      .map_or(Privacy::Shared, Privacy::Of),
  )
}

/// The account a room is private to, where one can be named: the owner's account for the owner's private room, the
/// owner account of the supervisor whose own room on the record it is, and none for every other room.
pub async fn private_account(receiver: &Receiver, chat_id: i64) -> Result<Option<i64>, NotReceived> {
  Ok(privacy(receiver, chat_id).await?.account())
}

/// Every private room, each to the account it is private to: the owner's private room to the owner's account, and
/// the own room on the record of each supervisor that names its owner account to that account. A group room stands
/// among none of them, however many of its members have left it.
pub async fn private_rooms(receiver: &Receiver) -> Result<BTreeMap<i64, i64>, NotReceived> {
  let mut rooms = BTreeMap::new();

  if let Some(account) = owner_account(receiver).await? {
    rooms.insert(receiver.owner_room(), account);
  }

  for agent in listed_designated(receiver).await? {
    if let Some(account) = agent.owner_account
      && let Some(room) = room_of_supervisor(receiver.db(), &agent.name).await?
    {
      rooms.insert(room, account);
    }
  }

  Ok(rooms)
}

/// The private rooms of an account: the own rooms of the supervisors it owns, and the owner's private room for the
/// owner's account.
pub async fn private_rooms_of(receiver: &Receiver, account: i64) -> Result<Vec<i64>, NotReceived> {
  Ok(
    private_rooms(receiver)
      .await?
      .into_iter()
      .filter_map(|(room, own)| (own == account).then_some(room))
      .collect(),
  )
}

/// Who an account's words in the console stand under: the owner's sender where the web unit links the account to
/// it, and the account's record identity otherwise, made where it is missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speaker {
  Owner,
  Account { sender: i64 },
}

/// The sender an account speaks as in the console.
pub async fn speaker(receiver: &Receiver, account: i64) -> Result<Speaker, NotReceived> {
  if owner_account(receiver).await? == Some(account) {
    return Ok(Speaker::Owner);
  }

  Ok(Speaker::Account {
    sender: identity(receiver.db(), account).await?,
  })
}

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
  /// A private room whose account is not known yet, so no account writes in it: the owner's private room while the
  /// web unit links no account to the owner's sender.
  Sealed,
}

impl Privacy {
  /// The account it is private to, where one is known.
  pub fn account(self) -> Option<i64> {
    match self {
      Self::Of(account) => Some(account),
      Self::Shared | Self::Sealed => None,
    }
  }

  /// Whether it is a private room whose account is not known yet.
  pub fn sealed(self) -> bool {
    self == Self::Sealed
  }

  // The privacy a kept row states: its account, or sealed where it names none.
  fn kept(account: Option<i64>) -> Self {
    account.map_or(Self::Sealed, Self::Of)
  }
}

/// What the record keeps of whose private room a room is: none for a room it keeps no row of.
pub async fn kept_privacy(db: &DatabaseConnection, chat_id: i64) -> Result<Option<Privacy>, Fault> {
  one(
    db,
    r#"SELECT "account" FROM "private_rooms" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await?
  .map(|row| row.try_get::<Option<i64>>("", "account").map(Privacy::kept))
  .transpose()
  .map_err(Fault::from)
}

/// Every room the record keeps as private, each with its privacy.
pub async fn kept_private_rooms(db: &DatabaseConnection) -> Result<BTreeMap<i64, Privacy>, Fault> {
  all(db, r#"SELECT "chat_id", "account" FROM "private_rooms" ORDER BY "chat_id""#, vec![])
    .await?
    .iter()
    .map(|row| {
      Ok((
        row.try_get::<i64>("", "chat_id")?,
        Privacy::kept(row.try_get::<Option<i64>>("", "account")?),
      ))
    })
    .collect::<Result<BTreeMap<i64, Privacy>, sea_orm::DbErr>>()
    .map_err(Fault::from)
}

/// A room kept as a private room: to `account`, or sealed where none is known. A sealed row is filled in once its
/// account is known, and an account once kept never changes or clears; whether the record changed.
pub async fn keep_private(
  db: &DatabaseConnection,
  chat_id: i64,
  account: Option<i64>,
) -> Result<bool, Fault> {
  let written = run(
    db,
    r#"INSERT INTO "private_rooms" ("chat_id", "account", "written_at") VALUES ($1, $2, $3)
       ON CONFLICT ("chat_id") DO UPDATE SET "account" = excluded."account", "written_at" = excluded."written_at"
         WHERE "private_rooms"."account" IS NULL AND excluded."account" IS NOT NULL"#,
    vec![chat_id.into(), account.into(), now_ms().into()],
  )
  .await?;

  Ok(written > 0)
}

// Whose private room a room is as the agency and the web unit say now, kept where they name it: the owner's private
// room the owner's account's, sealed while the web unit links none, and a supervisor's own room its owner
// account's.
async fn learned(receiver: &Receiver, chat_id: i64) -> Result<Privacy, NotReceived> {
  if chat_id == receiver.owner_room() {
    let account = owner_account(receiver).await?;

    keep_private(receiver.db(), chat_id, account).await?;

    return Ok(Privacy::kept(account));
  }

  let agents = listed_designated(receiver).await?;
  let Some(account) = supervisor_of_room(receiver.db(), chat_id, &agents)
    .await?
    .and_then(|agent| agent.owner_account)
  else {
    return Ok(Privacy::Shared);
  };

  keep_private(receiver.db(), chat_id, Some(account)).await?;

  Ok(Privacy::Of(account))
}

/// Whose private room a room is: the record's own row first, and where it keeps none or a sealed one, what the
/// agency and the web unit say now, kept for every read after. The owner's private room is private whatever they
/// answer, so it reads as sealed while they cannot say whose it is; any other room the record keeps nothing of
/// fails the read where they do not answer, rather than reading as no one's.
pub async fn privacy(receiver: &Receiver, chat_id: i64) -> Result<Privacy, NotReceived> {
  let kept = kept_privacy(receiver.db(), chat_id).await?;

  if let Some(Privacy::Of(account)) = kept {
    return Ok(Privacy::Of(account));
  }

  match learned(receiver, chat_id).await {
    Ok(privacy) => Ok(privacy),
    Err(_) if kept == Some(Privacy::Sealed) || chat_id == receiver.owner_room() => Ok(Privacy::Sealed),
    Err(why) => Err(why),
  }
}

/// The account a room is private to, where one is known.
pub async fn private_account(receiver: &Receiver, chat_id: i64) -> Result<Option<i64>, NotReceived> {
  Ok(privacy(receiver, chat_id).await?.account())
}

/// Every private room the agency and the web unit name now, kept where the record keeps it no account yet: the
/// owner's private room, sealed while no account is linked to his sender, and the own room of every supervisor that
/// names its owner account. Running it again changes nothing; what it cannot read stays as kept, and is asked again
/// by the reads after. How many rows it wrote.
pub async fn keep_known_private_rooms(receiver: &Receiver) -> Result<usize, NotReceived> {
  let mut written = 0;
  // The owner's private room is kept sealed where the web unit cannot say whose it is, and the supervisors are
  // read all the same; the first unit that did not answer is told once both are done.
  let owner = owner_account(receiver).await;

  if keep_private(receiver.db(), receiver.owner_room(), owner.as_ref().ok().copied().flatten()).await? {
    written += 1;
  }

  let agents = listed_designated(receiver).await;

  for agent in agents.as_deref().unwrap_or_default() {
    if let Some(account) = agent.owner_account
      && let Some(room) = room_of_supervisor(receiver.db(), &agent.name).await?
      && keep_private(receiver.db(), room, Some(account)).await?
    {
      written += 1;
    }
  }

  owner?;
  agents?;

  Ok(written)
}

/// The private rooms of an account the record keeps, after the private rooms the agency and the web unit name now
/// are kept where they answer: the own rooms of the supervisors it owns, and the owner's private room for the
/// owner's account.
pub async fn private_rooms_of(receiver: &Receiver, account: i64) -> Result<Vec<i64>, NotReceived> {
  if let Err(why) = keep_known_private_rooms(receiver).await {
    println!("bmo-receiver: account {account}'s private rooms are read as kept, since {why}");
  }

  Ok(
    kept_private_rooms(receiver.db())
      .await?
      .into_iter()
      .filter_map(|(room, privacy)| (privacy == Privacy::Of(account)).then_some(room))
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

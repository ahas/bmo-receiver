//! The console's accounts as the receiver keeps them (docs/receiver.md, 「Console rooms and accounts」): each
//! account's record identity, a room's members by account, and the private rooms one account alone speaks in. The
//! accounts themselves are the web unit's; the receiver keeps only what its rooms need of them.

use std::collections::BTreeMap;

use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};

use crate::receiver::wake::agents::Designated;
use crate::receiver::{NotReceived, Receiver};
use crate::roads::Road;
use crate::roads::record::{keep_sender_mark, next_sender};
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
pub async fn identity_of<C: ConnectionTrait>(db: &C, account: i64) -> Result<Option<i64>, Fault> {
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

/// The name a record identity goes by from now: given by `PUT /accounts/{account}/identity` and by every word, file
/// or forward of the account that carries one, so every reading of the person's name answers the latest of them.
pub async fn name_identity(db: &DatabaseConnection, sender: i64, name: &str) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "people_names" ("sender_id", "name", "named_at") VALUES ($1, $2, $3)
       ON CONFLICT ("sender_id") DO UPDATE SET "name" = EXCLUDED."name", "named_at" = EXCLUDED."named_at""#,
    vec![sender.into(), name.into(), now_ms().into()],
  )
  .await?;

  Ok(())
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

  /// The privacy a kept row states: its account, or sealed where it names none.
  pub fn kept(account: Option<i64>) -> Self {
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

/// Whether a room is the owner's private room: the one the configuration names, the one numbered as his sender, as
/// a chat service numbers his private chat, and the one a chat service says is his private chat with the agent,
/// whichever room the configuration names.
pub async fn owners_room(receiver: &Receiver, chat_id: i64) -> bool {
  chat_id == receiver.owner_room()
    || chat_id == receiver.owner_id()
    || receiver.roads().said_owner_room(chat_id).await
}

/// Whose private room a room is: the record's own row (docs/receiver.md, 「Console rooms and accounts」). No read
/// assigns a room to an account on what the agency lists: a BMO's own room is assigned by the addition that makes it
/// and by the one pass after the record began keeping rooms, and the owner's private room by the web unit's link of
/// his sender alone, asked only while the record keeps no account of it and kept once it answers. While the web
/// unit links none, or does not answer, the owner's private room is sealed.
pub async fn privacy(receiver: &Receiver, chat_id: i64) -> Result<Privacy, NotReceived> {
  let kept = kept_privacy(receiver.db(), chat_id).await?;

  if let Some(Privacy::Of(account)) = kept {
    return Ok(Privacy::Of(account));
  }

  if !owners_room(receiver, chat_id).await {
    return Ok(kept.unwrap_or(Privacy::Shared));
  }

  match owner_account(receiver).await {
    Ok(account) => {
      keep_private(receiver.db(), chat_id, account).await?;

      Ok(Privacy::kept(account))
    }
    Err(why) => {
      println!(
        "chat {chat_id}: the owner's private room reads sealed, since his account was not read: {why}"
      );

      Ok(Privacy::Sealed)
    }
  }
}

/// The account a room is private to, where one is known.
pub async fn private_account(receiver: &Receiver, chat_id: i64) -> Result<Option<i64>, NotReceived> {
  Ok(privacy(receiver, chat_id).await?.account())
}

/// The private rooms of an account the record keeps: the own rooms of the supervisors it owns, and the owner's
/// private room for the owner's account once the web unit links it.
pub async fn private_rooms_of(receiver: &Receiver, account: i64) -> Result<Vec<i64>, NotReceived> {
  privacy(receiver, receiver.owner_room()).await?;

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

/// The rooms the record keeps as the account's private rooms, in room order: never a sealed one, which names none.
pub async fn kept_private_rooms_of(db: &DatabaseConnection, account: i64) -> Result<Vec<i64>, Fault> {
  all(
    db,
    r#"SELECT "chat_id" FROM "private_rooms" WHERE "account" = $1 ORDER BY "chat_id""#,
    vec![account.into()],
  )
  .await?
  .iter()
  .map(|row| row.try_get::<i64>("", "chat_id").map_err(Fault::from))
  .collect()
}

/// What an account's removal takes beside its private rooms, each a number of rows (docs/receiver.md, 「An
/// account's removal」).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccountRows {
  /// Its rows as a member of a room that is not its private room.
  pub memberships: i64,
  /// Its record identity.
  pub identities: i64,
  /// The name its record identity goes by.
  pub names: i64,
  /// Its record identity muted in a room that is not its private room.
  pub mutes: i64,
  /// The pictures set on its record identity.
  pub pictures: i64,
}

// How many rows one count reads.
async fn counted(db: &DatabaseConnection, sql: &str, values: Vec<sea_orm::Value>) -> Result<i64, Fault> {
  Ok(
    one(db, sql, values)
      .await?
      .map(|row| row.try_get::<i64>("", "n"))
      .transpose()?
      .unwrap_or(0),
  )
}

/// What the account's removal would take beside its private rooms now, nothing written: a row in one of its private
/// rooms is that room's, and goes with it.
pub async fn account_rows(db: &DatabaseConnection, account: i64) -> Result<AccountRows, Fault> {
  let memberships = counted(
    db,
    r#"SELECT count(*) AS "n" FROM "room_accounts" WHERE "account" = $1
       AND "chat_id" NOT IN (SELECT "chat_id" FROM "private_rooms" WHERE "account" = $1)"#,
    vec![account.into()],
  )
  .await?;
  let Some(sender) = identity_of(db, account).await? else {
    return Ok(AccountRows {
      memberships,
      ..AccountRows::default()
    });
  };
  let names = counted(
    db,
    r#"SELECT count(*) AS "n" FROM "people_names" WHERE "sender_id" = $1"#,
    vec![sender.into()],
  )
  .await?;
  let mutes = counted(
    db,
    r#"SELECT count(*) AS "n" FROM "muted_members" WHERE "user_id" = $2
       AND "chat_id" NOT IN (SELECT "chat_id" FROM "private_rooms" WHERE "account" = $1)"#,
    vec![account.into(), sender.into()],
  )
  .await?;
  let pictures = counted(
    db,
    r#"SELECT (SELECT count(*) FROM "avatar_sets" WHERE "kind" = 'person' AND "subject_id" = $1)
            + (SELECT count(*) FROM "avatars" WHERE "kind" = 'person' AND "subject_id" = $1) AS "n""#,
    vec![sender.into()],
  )
  .await?;

  Ok(AccountRows {
    memberships,
    identities: 1,
    names,
    mutes,
    pictures,
  })
}

/// The account's rows beside its private rooms taken in one write: its memberships, and its record identity with
/// its name, its mutes and the pictures set on it, its number kept so it is never handed out again. Answers what was
/// taken and the paths of the pictures' files, which the caller takes down once the write stands.
pub async fn take_account_rows(
  db: &DatabaseConnection,
  account: i64,
) -> Result<(AccountRows, Vec<String>), Fault> {
  let written = db.begin().await.map_err(Fault::from)?;
  let mut taken = AccountRows {
    memberships: wrote(
      run(&written, r#"DELETE FROM "room_accounts" WHERE "account" = $1"#, vec![account.into()]).await?,
    ),
    ..AccountRows::default()
  };
  let mut files = vec![];

  if let Some(sender) = identity_of(&written, account).await? {
    taken.names = wrote(
      run(&written, r#"DELETE FROM "people_names" WHERE "sender_id" = $1"#, vec![sender.into()]).await?,
    );
    taken.mutes = wrote(
      run(&written, r#"DELETE FROM "muted_members" WHERE "user_id" = $1"#, vec![sender.into()]).await?,
    );

    for sql in [
      r#"DELETE FROM "avatar_sets" WHERE "kind" = 'person' AND "subject_id" = $1 RETURNING "path""#,
      r#"DELETE FROM "avatars" WHERE "kind" = 'person' AND "subject_id" = $1 RETURNING "path""#,
    ] {
      let gone = one(&written, sql, vec![sender.into()]).await?;

      if let Some(row) = gone {
        taken.pictures += 1;
        files.extend(row.try_get::<Option<String>>("", "path")?);
      }
    }

    keep_sender_mark(&written, sender).await?;
    taken.identities = wrote(
      run(
        &written,
        r#"DELETE FROM "provider_users" WHERE "provider" = $1 AND "user_id" = $2"#,
        vec![Road::Record.key().into(), identity_key(account).into()],
      )
      .await?,
    );
  }

  written.commit().await.map_err(Fault::from)?;

  Ok((taken, files))
}

// A count of rows written, as the answer numbers it.
fn wrote(rows: u64) -> i64 {
  i64::try_from(rows).unwrap_or(i64::MAX)
}

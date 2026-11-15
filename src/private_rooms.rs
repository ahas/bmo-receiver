//! Whose private room each room is, settled on the record at start and shown by the dry run without writing
//! (docs/receiver.md, 「Console rooms and accounts」). The owner's private room is assigned from the web unit's link
//! of his sender alone. A BMO's own room is assigned from the agency's listing once, on the first start that reads it
//! after the record began keeping private rooms; every start after only seals the own rooms no registered supervisor
//! claims, and never assigns one.

use receiver_ports::agency::Agency;
use receiver_ports::web::Web;
use sea_orm::DatabaseConnection;

use crate::accounts::{Privacy, keep_private, kept_private_rooms};
use crate::receiver::Receiver;
use crate::receiver::wake::agents::Designated;
use crate::roads::Road;
use crate::store::{Fault, settings};
use crate::{all, one};

/// The setting that marks the one pass adopting what the agency names as done.
pub const ADOPTED: &str = "private_rooms_adopted";

/// The units a settling may ask beside the store, each read and never written; one left out is not asked.
#[derive(Clone, Copy, Default)]
pub struct Asked<'a> {
  pub agency: Option<&'a dyn Agency>,
  pub web: Option<&'a dyn Web>,
}

/// What a room is settled as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
  /// As the record keeps it already.
  Kept(Privacy),
  /// Kept from now as the account's private room.
  Assign(i64),
  /// Kept from now as a private room no account writes in.
  Seal,
  /// No private room.
  Shared,
  /// Left as it stands, since what would settle it was not read.
  Unknown,
}

/// One room as the settling reads it: its number, its title where it has one, what it is settled as, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settled {
  pub room: i64,
  pub title: Option<String>,
  pub verdict: Verdict,
  pub why: String,
}

// Every room the store holds anything of, with its title, its name on the record where it stands under one of its
// own, and whether that name is a supervisor's: one a number of the record stands under.
struct Held {
  room: i64,
  title: Option<String>,
  own_name: Option<String>,
}

// Whether the store holds the private rooms' table yet: a store from before it is read as keeping none.
async fn kept_at_all(db: &DatabaseConnection) -> Result<bool, Fault> {
  let row = one(db, r#"SELECT to_regclass('"private_rooms"') IS NOT NULL AS "stands""#, vec![]).await?;

  Ok(row.map(|row| row.try_get::<bool>("", "stands")).transpose()?.unwrap_or(false))
}

async fn held(db: &DatabaseConnection, kept_at_all: bool) -> Result<Vec<Held>, Fault> {
  let kept = if kept_at_all { r#" UNION SELECT "chat_id" FROM "private_rooms""# } else { "" };
  let rows = all(
    db,
    &format!(
      r#"SELECT r."chat_id", t."title",
         (SELECT p."room_id" FROM "provider_rooms" p
            WHERE p."provider" = $1 AND p."chat_id" = r."chat_id" AND p."room_id" <> r."chat_id"::text
              AND p."room_id" NOT LIKE 'owner-room:%'
              AND EXISTS (SELECT 1 FROM "provider_users" u WHERE u."provider" = $1 AND u."user_id" = p."room_id"))
           AS "own_name"
       FROM (SELECT "chat_id" FROM "chat_titles" UNION SELECT "chat_id" FROM "provider_rooms"
             UNION SELECT DISTINCT "chat_id" FROM "messages"{kept}) r
       LEFT JOIN "chat_titles" t ON t."chat_id" = r."chat_id"
       ORDER BY r."chat_id""#
    ),
    vec![Road::Record.key().into()],
  )
  .await?;

  rows
    .iter()
    .map(|row| {
      Ok(Held {
        room: row.try_get("", "chat_id")?,
        title: row.try_get("", "title")?,
        own_name: row.try_get("", "own_name")?,
      })
    })
    .collect::<Result<Vec<Held>, sea_orm::DbErr>>()
    .map_err(Fault::from)
}

// The owner's private room, settled from the web unit's link of his sender alone.
async fn owners(kept: Option<Privacy>, owner: i64, web: Option<&dyn Web>) -> (Verdict, String) {
  if let Some(Privacy::Of(account)) = kept {
    return (Verdict::Kept(Privacy::Of(account)), "the owner's private room, kept".to_owned());
  }

  let Some(web) = web else {
    return (
      Verdict::Seal,
      "the owner's private room; the web unit was not asked, so it is sealed".to_owned(),
    );
  };

  match web.user_of_sender(owner).await {
    Ok(Some(user)) => match user.trim().parse::<i64>() {
      Ok(account) => {
        (Verdict::Assign(account), format!("the web unit links the owner's sender to {account}"))
      }
      Err(_) => (Verdict::Seal, format!("the web unit links the owner's sender to {user}, no account")),
    },
    Ok(None) => (
      Verdict::Seal,
      "the owner's private room; no account is linked to his sender yet".to_owned(),
    ),
    Err(why) => (Verdict::Seal, format!("the owner's private room; the web unit did not answer: {why}")),
  }
}

// A supervisor's own room the record keeps nothing of, settled from the agency's listing: assigned to the owner it
// names only by the one pass that adopts, sealed where no supervisor claims it, no one's otherwise.
fn own_room(name: &str, agents: Option<&[Designated]>, adopting: bool) -> (Verdict, String) {
  let Some(agents) = agents else {
    return (
      Verdict::Unknown,
      format!("{name}'s own room; the agency was not read, so nothing is written"),
    );
  };

  let Some(agent) = agents.iter().find(|agent| agent.name == name) else {
    return (Verdict::Seal, format!("{name}'s own room, which no registered supervisor claims"));
  };
  let (account, whence) = match (agent.owner_account, owner_from_name(name)) {
    (Some(account), _) => (account, "its owner"),
    (None, Some(account)) => (account, "its owner, owner from name"),
    (None, None) => return (Verdict::Shared, format!("{name} names no owner")),
  };

  if adopting {
    (Verdict::Assign(account), format!("{name} names {account} as {whence}"))
  } else {
    (
      Verdict::Shared,
      format!("{name} names {account} as {whence}, which only its addition or the first pass assigns"),
    )
  }
}

/// The account a person's own BMO is named for: `u<n>`, `n` a positive number written without a leading zero, the
/// name the console gives the BMO it stands for account `n` (the web unit's `docs/accounts.md`). Registrations made
/// before the receiver handed the agency the owner account name none, so the adopting pass reads the account off
/// such a name; the name can only give a room to the account it names. Any other name names no account.
pub fn owner_from_name(name: &str) -> Option<i64> {
  let digits = name.strip_prefix('u')?;

  if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|digit| digit.is_ascii_digit()) {
    return None;
  }

  digits.parse::<i64>().ok().filter(|account| *account > 0)
}

/// Every room the store holds, settled: written where `write` asks it, and only read otherwise, a store from before
/// the private rooms were kept read as keeping none. The agency and the web unit are asked where `asked` names them,
/// each read and never written; a room what was left out would settle is shown as it would stand without it.
/// Writing, a read of the agency marks the one adopting pass done.
pub async fn settle(
  db: &DatabaseConnection,
  (owner, owner_room): (i64, i64),
  asked: Asked<'_>,
  write: bool,
) -> Result<Vec<Settled>, Fault> {
  let kept_at_all = kept_at_all(db).await?;

  if write && !kept_at_all {
    return Err(Fault::new("the store keeps no private rooms yet: its migrations have not run"));
  }

  let kept = if kept_at_all { kept_private_rooms(db).await? } else { Default::default() };
  let adopting = settings::get(db, ADOPTED).await?.is_none();
  let agents = match asked.agency {
    Some(agency) => match agency.supervisors().await {
      Ok(listed) => Some(listed.agents.iter().map(|agent| Designated::of(agent, "")).collect::<Vec<_>>()),
      Err(why) => {
        println!("bmo-receiver: the agency was not read while the private rooms settled: {why}");
        None
      }
    },
    None => None,
  };
  let mut settled = Vec::new();

  let mut rooms = held(db, kept_at_all).await?;

  // The owner's private room is settled whether or not the store holds anything of it yet.
  for room in [owner_room, owner] {
    if !rooms.iter().any(|held| held.room == room) {
      rooms.push(Held {
        room,
        title: None,
        own_name: None,
      });
    }
  }

  for room in rooms {
    let now = kept.get(&room.room).copied();
    let (verdict, why) = if room.room == owner_room || room.room == owner {
      owners(now, owner, asked.web).await
    } else if let Some(privacy) = now {
      (Verdict::Kept(privacy), "kept on the room's record".to_owned())
    } else if let Some(name) = &room.own_name {
      own_room(name, agents.as_deref(), adopting)
    } else {
      (Verdict::Shared, "no private room's rule names it".to_owned())
    };

    if write {
      match verdict {
        Verdict::Assign(account) => {
          keep_private(db, room.room, Some(account)).await?;
        }
        Verdict::Seal => {
          keep_private(db, room.room, None).await?;
        }
        Verdict::Kept(_) | Verdict::Shared | Verdict::Unknown => {}
      }
    }

    settled.push(Settled {
      room: room.room,
      title: room.title,
      verdict,
      why,
    });
  }

  if write && adopting && agents.is_some() {
    settings::set(db, ADOPTED, "done").await?;
  }

  Ok(settled)
}

/// The settling the service runs as it starts, asking the agency and the web unit it stands on.
pub async fn at_start(receiver: &Receiver) -> Result<Vec<Settled>, Fault> {
  let units = receiver.units();
  let asked = Asked {
    agency: Some(units.agency.as_ref()),
    web: Some(units.web.as_ref()),
  };

  settle(receiver.db(), (receiver.owner_id(), receiver.owner_room()), asked, true).await
}

impl Settled {
  /// The room as the dry run prints it: its number, what it is settled as, its title and why.
  pub fn line(&self) -> String {
    let verdict = match &self.verdict {
      Verdict::Kept(Privacy::Of(account)) => format!("private to {account}, kept"),
      Verdict::Kept(Privacy::Sealed) => "sealed, kept".to_owned(),
      Verdict::Kept(Privacy::Shared) | Verdict::Shared => "shared".to_owned(),
      Verdict::Assign(account) => format!("private to {account}, to be kept"),
      Verdict::Seal => "sealed, to be kept".to_owned(),
      Verdict::Unknown => "unknown".to_owned(),
    };

    format!("{}\t{verdict}\t{}\t{}", self.room, self.title.as_deref().unwrap_or("-"), self.why)
  }
}

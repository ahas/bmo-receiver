//! A room's senders matched to its members (docs/messages.md, 「A sender matched to the room's members」): a sender
//! a road numbered when it read the room another way holds no profile picture, since the road keeps its pictures
//! under the number it gives the person now. As the service starts, each room carrying such a sender has its own
//! road's member list asked once, and a sender whose nickname names exactly one member moves to that member's
//! number in that room, so a message's face and the member sheet ask for one person by one number.

use std::collections::{BTreeMap, BTreeSet};

use sea_orm::TransactionTrait;

use super::{Receiver, reads};
use crate::roads::Road;
use crate::{Fault, all, run};

/// What became of a room's senders matched to its member list.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Matched {
  /// Each sender moved: its old number, the member's number and how many messages moved with it.
  pub moved: Vec<(i64, i64, u64)>,
  /// The nicknames more than one member answers to, or a sender's several nicknames name different members: no
  /// sender of them moved.
  pub ambiguous: Vec<String>,
}

// The senders of the words that came on a road in a room, each with every nickname its words stand under, the
// owner's number and every number an agent's own words stand under left out.
async fn senders_of(
  receiver: &Receiver,
  room: i64,
  key: &str,
) -> Result<BTreeMap<i64, BTreeSet<String>>, Fault> {
  let rows = all(
    receiver.db(),
    r#"SELECT DISTINCT "sender_id", "sender_name" FROM "messages"
       WHERE "chat_id" = $1 AND "via" = $2 AND "sender_id" IS NOT NULL AND "sender_id" <> $3
         AND "sender_id" NOT IN (SELECT "sender_id" FROM "messages" WHERE "agent_name" IS NOT NULL
                                   AND "sender_id" IS NOT NULL)"#,
    vec![room.into(), key.into(), receiver.owner_id().into()],
  )
  .await?;
  let mut senders: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();

  for row in &rows {
    let sender: i64 = row.try_get("", "sender_id")?;
    let name: String = row.try_get("", "sender_name")?;
    let name = name.trim();

    if !name.is_empty() {
      senders.entry(sender).or_default().insert(name.to_owned());
    }
  }

  Ok(senders)
}

// One sender's messages, mute and reactions in a room moved to a member's number in one write: how many messages
// moved. A mute or reaction the member's number already holds stands, and the sender's own is let go.
async fn moved(receiver: &Receiver, room: i64, from: i64, to: i64) -> Result<u64, Fault> {
  let tx = receiver.db().begin().await.map_err(Fault::from)?;
  let messages = run(
    &tx,
    r#"UPDATE "messages" SET "sender_id" = $3 WHERE "chat_id" = $1 AND "sender_id" = $2"#,
    vec![room.into(), from.into(), to.into()],
  )
  .await?;

  run(
    &tx,
    r#"UPDATE "muted_members" SET "user_id" = $3 WHERE "chat_id" = $1 AND "user_id" = $2
         AND NOT EXISTS (SELECT 1 FROM "muted_members" WHERE "chat_id" = $1 AND "user_id" = $3)"#,
    vec![room.into(), from.into(), to.into()],
  )
  .await?;
  run(
    &tx,
    r#"DELETE FROM "muted_members" WHERE "chat_id" = $1 AND "user_id" = $2"#,
    vec![room.into(), from.into()],
  )
  .await?;
  run(
    &tx,
    r#"UPDATE "message_reactions" AS "r" SET "by" = $3 WHERE "r"."chat_id" = $1 AND "r"."by" = $2
         AND NOT EXISTS (SELECT 1 FROM "message_reactions" AS "o" WHERE "o"."chat_id" = $1
                           AND "o"."message_id" = "r"."message_id" AND "o"."by" = $3)"#,
    vec![room.into(), from.to_string().into(), to.to_string().into()],
  )
  .await?;
  run(
    &tx,
    r#"DELETE FROM "message_reactions" WHERE "chat_id" = $1 AND "by" = $2"#,
    vec![room.into(), from.to_string().into()],
  )
  .await?;
  tx.commit().await.map_err(Fault::from)?;

  Ok(messages)
}

/// A room's senders matched to the member list its own road gives: each sender of the words that came on that
/// road, neither the owner nor an agent nor already a member, moved to the one member of its nickname. None where
/// the room stands on no chat service, or its road lists no members, holds none of the room or does not answer.
pub async fn matched(receiver: &Receiver, room: i64) -> Option<Matched> {
  let Some(Road::Service(key)) = receiver.roads().own(room).await else { return None };
  let road = receiver.roads().chats().road(&key)?.clone();
  let members = match road.members(room).await {
    Ok(listed) => listed.members,
    Err(why) => {
      eprintln!("bmo-receiver: the {key} road did not list the members of room {room}: {why}");

      return None;
    }
  };
  let senders = senders_of(receiver, room, &key).await.unwrap_or_else(|why| {
    eprintln!("bmo-receiver: the senders of room {room} were not read: {why}");

    BTreeMap::new()
  });
  let numbered: BTreeSet<i64> = members.iter().filter_map(|member| member.person).collect();
  let mut by_name: BTreeMap<&str, Vec<Option<i64>>> = BTreeMap::new();

  for member in &members {
    by_name.entry(member.name.trim()).or_default().push(member.person);
  }

  let mut done = Matched::default();

  for (sender, names) in senders {
    if numbered.contains(&sender)
      || reads::owner_of_sender(receiver.db(), &key, room, sender).await.unwrap_or(true)
    {
      continue;
    }

    let mut answers: BTreeSet<Option<i64>> = BTreeSet::new();
    let mut doubled = vec![];

    for name in &names {
      match by_name.get(name.as_str()).map(Vec::as_slice) {
        Some([one]) => {
          answers.insert(*one);
        }
        Some([_, _, ..]) => doubled.push(name.clone()),
        _ => {}
      }
    }

    if !doubled.is_empty() || answers.len() > 1 {
      let named: Vec<String> = if doubled.is_empty() { names.into_iter().collect() } else { doubled };

      eprintln!(
        "bmo-receiver: sender {sender} of room {room} was not matched to a member: more than one member answers to {}",
        named.join(", ")
      );
      done.ambiguous.extend(named);

      continue;
    }

    let Some(Some(member)) = answers.into_iter().next() else { continue };

    match moved(receiver, room, sender, member).await {
      Ok(messages) => {
        eprintln!(
          "bmo-receiver: sender {sender} of room {room} matched member {member}: {messages} messages moved"
        );
        done.moved.push((sender, member, messages));
      }
      Err(why) => {
        eprintln!("bmo-receiver: sender {sender} of room {room} was not moved to member {member}: {why}")
      }
    }
  }

  Some(done)
}

// The rooms whose own road carried words from a sender that holds no profile picture, neither the owner nor an
// agent.
async fn faceless(receiver: &Receiver) -> Result<Vec<i64>, Fault> {
  let rows = all(
    receiver.db(),
    r#"SELECT DISTINCT "m"."chat_id" FROM "messages" AS "m"
       WHERE "m"."via" IS NOT NULL AND "m"."sender_id" IS NOT NULL AND "m"."sender_id" <> $1
         AND "m"."sender_id" NOT IN (SELECT "sender_id" FROM "messages" WHERE "agent_name" IS NOT NULL
                                       AND "sender_id" IS NOT NULL)
         AND NOT EXISTS (SELECT 1 FROM "avatars" AS "a" WHERE "a"."kind" = 'person'
                           AND "a"."subject_id" = "m"."sender_id" AND "a"."path" IS NOT NULL AND "a"."path" <> '')
         AND NOT EXISTS (SELECT 1 FROM "avatar_sets" AS "s" WHERE "s"."kind" = 'person'
                           AND "s"."subject_id" = "m"."sender_id")
       ORDER BY 1"#,
    vec![receiver.owner_id().into()],
  )
  .await?;

  Ok(rows.iter().filter_map(|row| row.try_get::<i64>("", "chat_id").ok()).collect())
}

/// Every room carrying a sender that holds no profile picture matched to its member list, as the service starts:
/// how many senders moved.
pub async fn match_every(receiver: &Receiver) -> usize {
  let rooms = faceless(receiver).await.unwrap_or_else(|why| {
    eprintln!("bmo-receiver: the rooms whose senders hold no picture were not read: {why}");

    vec![]
  });
  let mut moved = 0;

  for room in rooms {
    if let Some(done) = matched(receiver, room).await {
      moved += done.moved.len();
    }
  }

  moved
}

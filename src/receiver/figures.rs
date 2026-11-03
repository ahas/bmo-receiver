//! What a room is made of, counted off the receiver's own record as the console asks for it (`docs/boundary-api.md`,
//! 「A room's figures」): its messages, the people who said them and the hours they were said in, and each agent's
//! own share of them. Nothing here is kept; every figure is counted when it is asked for.

use sea_orm::{DatabaseConnection, Value};

use crate::{Fault, all, one};

/// How a room's record tells an agent's words: the name its words are filed under, where it has one, and the
/// numbers its roads gave it.
#[derive(Clone, Debug, Default)]
pub struct Filed {
  pub name: Option<String>,
  pub numbers: Vec<i64>,
}

impl Filed {
  // A condition over the room's rows as `m` that holds for a message so filed, its values numbered from `first`.
  fn condition(&self, first: usize) -> (String, Vec<Value>) {
    let mut parts: Vec<String> = vec![];
    let mut values: Vec<Value> = vec![];

    if let Some(name) = &self.name {
      parts.push(format!(r#""m"."sender_name" = ${first}"#));
      values.push(name.clone().into());
    }

    if !self.numbers.is_empty() {
      let at = first + values.len();
      let named: Vec<String> = (at..at + self.numbers.len()).map(|n| format!("${n}")).collect();

      parts.push(format!(r#""m"."sender_id" IN ({})"#, named.join(", ")));
      values.extend(self.numbers.iter().map(|number| Value::from(*number)));
    }

    if parts.is_empty() {
      return ("FALSE".to_owned(), values);
    }

    (format!("COALESCE(({}), FALSE)", parts.join(" OR ")), values)
  }
}

/// A room's messages counted: all of them, those people said, how many people said them, the first and last
/// sent and the last an agent filed, in epoch milliseconds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counted {
  pub messages: i64,
  pub from_people: i64,
  pub speakers: i64,
  pub first_at: Option<i64>,
  pub last_at: Option<i64>,
  pub last_answer_at: Option<i64>,
}

// One person among a room's senders: their number, or the name they spoke under where no number stands.
const SPEAKER: &str = r#"COALESCE(CAST("m"."sender_id" AS text), 'name:' || "m"."sender_name")"#;

/// Whether the database knows a time zone by that IANA name.
pub async fn zone_known(db: &DatabaseConnection, zone: &str) -> Result<bool, Fault> {
  let row =
    one(db, r#"SELECT 1 AS "one" FROM pg_timezone_names WHERE "name" = $1"#, vec![zone.into()]).await?;

  Ok(row.is_some())
}

/// A room's messages counted, an agent's words told as `agents` files them.
pub async fn counted(db: &DatabaseConnection, chat_id: i64, agents: &Filed) -> Result<Counted, Fault> {
  let (filed, values) = agents.condition(2);
  let sql = format!(
    r#"SELECT COUNT(*) AS "messages",
         COUNT(*) FILTER (WHERE NOT {filed}) AS "from_people",
         COUNT(DISTINCT CASE WHEN NOT {filed} THEN {SPEAKER} END) AS "speakers",
         MIN("m"."sent_at") AS "first_at", MAX("m"."sent_at") AS "last_at",
         MAX("m"."sent_at") FILTER (WHERE {filed}) AS "last_answer_at"
       FROM "messages" "m" WHERE "m"."chat_id" = $1"#
  );
  let Some(row) = one(db, &sql, [vec![chat_id.into()], values].concat()).await? else {
    return Ok(Counted::default());
  };

  Ok(Counted {
    messages: row.try_get("", "messages")?,
    from_people: row.try_get("", "from_people")?,
    speakers: row.try_get("", "speakers")?,
    first_at: row.try_get("", "first_at")?,
    last_at: row.try_get("", "last_at")?,
    last_answer_at: row.try_get("", "last_answer_at")?,
  })
}

/// The people who said most in a room, at most `count` of them, each under the name they last spoke under, with
/// how many messages they said; the most recent speaker first among equals.
pub async fn top(
  db: &DatabaseConnection,
  chat_id: i64,
  agents: &Filed,
  count: i64,
) -> Result<Vec<(String, i64)>, Fault> {
  let (filed, values) = agents.condition(3);
  let sql = format!(
    r#"SELECT (ARRAY_AGG("m"."sender_name" ORDER BY "m"."sent_at" DESC, "m"."rowid" DESC))[1] AS "name",
         COUNT(*) AS "messages"
       FROM "messages" "m" WHERE "m"."chat_id" = $1 AND NOT {filed}
       GROUP BY {SPEAKER} ORDER BY COUNT(*) DESC, MAX("m"."sent_at") DESC LIMIT $2"#
  );

  all(db, &sql, [vec![chat_id.into(), count.into()], values].concat())
    .await?
    .iter()
    .map(|row| Ok((row.try_get("", "name")?, row.try_get("", "messages")?)))
    .collect()
}

/// A room's messages by the hour of the day they were sent in a time zone, from midnight on.
pub async fn by_hour(db: &DatabaseConnection, chat_id: i64, zone: &str) -> Result<[i64; 24], Fault> {
  let rows = all(
    db,
    r#"SELECT CAST(EXTRACT(HOUR FROM (to_timestamp("m"."sent_at" / 1000.0) AT TIME ZONE $2)) AS bigint) AS "hour",
         COUNT(*) AS "messages"
       FROM "messages" "m" WHERE "m"."chat_id" = $1 GROUP BY 1"#,
    vec![chat_id.into(), zone.into()],
  )
  .await?;
  let mut hours = [0_i64; 24];

  for row in rows {
    let hour: i64 = row.try_get("", "hour")?;

    if let Some(held) = usize::try_from(hour).ok().and_then(|hour| hours.get_mut(hour)) {
      *held = row.try_get("", "messages")?;
    }
  }

  Ok(hours)
}

/// How many messages one agent filed in a room, and when it filed the last.
pub async fn filed(
  db: &DatabaseConnection,
  chat_id: i64,
  agent: &Filed,
) -> Result<(i64, Option<i64>), Fault> {
  let (filed, values) = agent.condition(2);
  let sql = format!(
    r#"SELECT COUNT(*) AS "messages", MAX("m"."sent_at") AS "last_at"
       FROM "messages" "m" WHERE "m"."chat_id" = $1 AND {filed}"#
  );
  let Some(row) = one(db, &sql, [vec![chat_id.into()], values].concat()).await? else {
    return Ok((0, None));
  };

  Ok((row.try_get("", "messages")?, row.try_get("", "last_at")?))
}

/// The agents answering in a room, by name.
pub async fn answerers(db: &DatabaseConnection, chat_id: i64) -> Result<Vec<String>, Fault> {
  all(
    db,
    r#"SELECT "supervisor" FROM "room_supervisors" WHERE "chat_id" = $1 ORDER BY "supervisor""#,
    vec![chat_id.into()],
  )
  .await?
  .iter()
  .map(|row| Ok(row.try_get("", "supervisor")?))
  .collect()
}

/// The sessions the record says have been given a room's context, newest first.
pub async fn sessions_given(db: &DatabaseConnection, chat_id: i64) -> Result<Vec<String>, Fault> {
  all(
    db,
    r#"SELECT "session_id" FROM "bmo_session_rooms" WHERE "chat_id" = $1 ORDER BY "rowid" DESC"#,
    vec![chat_id.into()],
  )
  .await?
  .iter()
  .map(|row| Ok(row.try_get("", "session_id")?))
  .collect()
}

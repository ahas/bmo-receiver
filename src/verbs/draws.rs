//! The fair draw (docs/random-draw.md): a draw made in a room — the request read and refused, the room's
//! committed seed consumed in one write, the line posted, the result handed back at once or to the next turn
//! — and a draw checked, out of the archive by its number or out of what its room line published alone.
//!
//! The method is the one the receiver's own tool draws by, so a draw made by either is checked by the other:
//! block c of a draw's stream is SHA-256 of `seed:draw:c`, read four bytes at a time as unsigned big-endian
//! words.

use bmo_chat_client::{ChatSend, ChatSendRequest};
use receiver_codes::{DrawAsked, DrawLine, DrawResult, FixedLine};
use sea_orm::{DatabaseConnection, TransactionTrait};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::chat::instructions::{Queued, queue};
use crate::receiver::wake;
use crate::verbs::VerbError;
use crate::verbs::sends::{NotSent, Sends};
use crate::{Fault, now_ms, one, run};

/// The method's name, published with every draw; a change to how the stream is read is a new one.
pub const DRAW_METHOD: &str = "bmo-draw-v1";

const SHAPES: [&str; 4] = ["dice", "range", "pick", "shuffle"];
const MODES: [&str; 2] = ["sync", "async"];
const SIDES: (i64, i64) = (2, 1000);
const COUNT: (i64, i64) = (1, 100);
const MODIFIER: i64 = 10_000;
const ITEMS: usize = 1000;
const ITEM_LENGTH: usize = 200;
const LABEL_LENGTH: usize = 200;
const WORD_SPAN: i64 = 1 << 32;

// The largest whole number a request's field may be, as the tool reads one.
const SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// A draw's request: the spec and what the draw decides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
  pub spec: Spec,
  pub label: Option<String>,
}

/// What a draw is asked to draw, in the order its record keeps the fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "shape", rename_all = "lowercase")]
pub enum Spec {
  Dice {
    sides: i64,
    count: i64,
    modifier: i64,
  },
  Range {
    min: i64,
    max: i64,
  },
  Pick {
    items: Vec<String>,
    take: i64,
  },
  Shuffle {
    items: Vec<String>,
  },
}

impl Spec {
  fn shape(&self) -> &'static str {
    match self {
      Self::Dice { .. } => "dice",
      Self::Range { .. } => "range",
      Self::Pick { .. } => "pick",
      Self::Shuffle { .. } => "shuffle",
    }
  }
}

/// What a draw came to, in the order its record keeps the fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "shape", rename_all = "lowercase")]
pub enum Outcome {
  Dice { faces: Vec<i64>, total: i64 },
  Range { value: i64 },
  Pick { picked: Vec<String> },
  Shuffle { order: Vec<String> },
}

// Which fields each shape reads. A field another shape reads is refused rather than ignored.
fn own_fields(shape: &str) -> &'static [&'static str] {
  match shape {
    "dice" => &["count", "sides", "modifier"],
    "range" => &["min", "max"],
    "pick" => &["items", "take"],
    _ => &["items"],
  }
}

const ALL_FIELDS: [&str; 7] = ["count", "sides", "modifier", "min", "max", "items", "take"];

// A string's length as the tool counts it, in UTF-16 units.
fn length(text: &str) -> usize {
  text.encode_utf16().count()
}

fn present<'a>(args: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
  args.get(name).filter(|value| !value.is_null())
}

fn integer_field(args: &Map<String, Value>, name: &str) -> Result<Option<i64>, String> {
  let Some(value) = present(args, name) else { return Ok(None) };
  let whole = value.as_i64().or_else(|| {
    value
      .as_f64()
      .filter(|number| number.fract() == 0.0 && number.abs() <= SAFE_INTEGER)
      .map(|number| number as i64)
  });

  whole.map(Some).ok_or_else(|| format!("{name} must be a whole number"))
}

fn items_field(args: &Map<String, Value>, least: usize) -> Result<Vec<String>, String> {
  let Some(items) = args.get("items").and_then(Value::as_array) else {
    return Err("items must be a list of strings".to_owned());
  };

  if items.len() < least || items.len() > ITEMS {
    return Err(format!("items must hold {least} to {ITEMS} entries"));
  }

  items
    .iter()
    .map(|item| match item.as_str() {
      Some(item) if !item.trim().is_empty() && length(item) <= ITEM_LENGTH => Ok(item.to_owned()),
      _ => Err(format!("every item must be a non-empty string of at most {ITEM_LENGTH} characters")),
    })
    .collect()
}

fn within(value: i64, least: i64, most: i64, name: &str) -> Result<(), String> {
  if value < least || value > most {
    Err(format!("{name} must be from {least} to {most}"))
  } else {
    Ok(())
  }
}

/// A draw's request read out of a call's arguments, or the sentence it is refused with: `shape` names what
/// is drawn; `count` defaults to one die, `modifier` to none and `take` to one item.
pub fn read_request(args: &Map<String, Value>) -> Result<Request, String> {
  let shape = args.get("shape").and_then(Value::as_str).filter(|shape| SHAPES.contains(shape));
  let Some(shape) = shape else {
    return Err(format!("shape must be one of {}", SHAPES.join(", ")));
  };
  let own = own_fields(shape);
  let stray: Vec<&str> = ALL_FIELDS
    .iter()
    .copied()
    .filter(|field| !own.contains(field) && present(args, field).is_some())
    .collect();

  if !stray.is_empty() {
    return Err(format!(
      "{} {} not belong to a {shape} draw",
      stray.join(", "),
      if stray.len() == 1 { "does" } else { "do" },
    ));
  }

  let label = match present(args, "label") {
    None => None,
    Some(Value::String(label)) if length(label) <= LABEL_LENGTH => {
      Some(label.trim().to_owned()).filter(|label| !label.is_empty())
    }
    Some(_) => return Err(format!("label must be a string of at most {LABEL_LENGTH} characters")),
  };

  Ok(Request {
    spec: read_spec(shape, args)?,
    label,
  })
}

fn read_spec(shape: &str, args: &Map<String, Value>) -> Result<Spec, String> {
  match shape {
    "dice" => {
      let sides = integer_field(args, "sides")?;
      let count = integer_field(args, "count")?;
      let modifier = integer_field(args, "modifier")?;
      let Some(sides) = sides else { return Err("a dice draw needs sides".to_owned()) };
      let (count, modifier) = (count.unwrap_or(1), modifier.unwrap_or(0));

      within(sides, SIDES.0, SIDES.1, "sides")?;
      within(count, COUNT.0, COUNT.1, "count")?;
      within(modifier, -MODIFIER, MODIFIER, "modifier")?;

      Ok(Spec::Dice {
        sides,
        count,
        modifier,
      })
    }
    "range" => {
      let min = integer_field(args, "min")?;
      let max = integer_field(args, "max")?;
      let (Some(min), Some(max)) = (min, max) else {
        return Err("a range draw needs min and max".to_owned());
      };

      if min > max {
        return Err("min must not be above max".to_owned());
      }

      if i128::from(max) - i128::from(min) + 1 > i128::from(WORD_SPAN) {
        return Err(format!("a range may hold at most {WORD_SPAN} values"));
      }

      Ok(Spec::Range { min, max })
    }
    "pick" => {
      let items = items_field(args, 1)?;
      let take = integer_field(args, "take")?.unwrap_or(1);

      within(take, 1, items.len() as i64, "take")?;

      Ok(Spec::Pick { items, take })
    }
    _ => Ok(Spec::Shuffle {
      items: items_field(args, 2)?,
    }),
  }
}

/// Whether a seed is 32 bytes written as 64 lowercase hexadecimal characters.
pub fn is_seed(text: &str) -> bool {
  text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn hex(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A fresh seed out of the system's cryptographic random source.
pub fn fresh_seed() -> String {
  hex(&rand::random::<[u8; 32]>())
}

/// The commitment to a seed: SHA-256 of its hexadecimal text, in lowercase hexadecimal.
pub fn commitment_of(seed: &str) -> String {
  hex(&Sha256::digest(seed.as_bytes()))
}

/// The words a seed and a draw's number give.
pub struct Stream {
  seed: String,
  draw_id: i64,
  counter: u64,
  block: Vec<u8>,
  at: usize,
}

impl Stream {
  pub fn new(seed: &str, draw_id: i64) -> Self {
    Self {
      seed: seed.to_owned(),
      draw_id,
      counter: 0,
      block: vec![],
      at: 0,
    }
  }

  /// The next 32-bit word, each call one further along.
  pub fn word(&mut self) -> u32 {
    if self.at >= self.block.len() {
      self.block =
        Sha256::digest(format!("{}:{}:{}", self.seed, self.draw_id, self.counter).as_bytes()).to_vec();
      self.counter += 1;
      self.at = 0;
    }

    let word =
      u32::from_be_bytes(self.block[self.at..self.at + 4].try_into().expect("a word is four bytes"));

    self.at += 4;
    word
  }

  /// A uniform integer below n, rejecting the words past the last whole multiple of n.
  pub fn below(&mut self, n: i64) -> i64 {
    let limit = (WORD_SPAN / n) * n;

    loop {
      let word = i64::from(self.word());

      if word < limit {
        return word % n;
      }
    }
  }

  fn shuffled(&mut self, items: &[String]) -> Vec<String> {
    let mut order = items.to_vec();

    for i in (1..order.len()).rev() {
      let j = self.below(i as i64 + 1) as usize;

      order.swap(i, j);
    }

    order
  }
}

/// The draw a spec makes out of a seed and a draw's number, read from the stream's start.
pub fn draw_with(spec: &Spec, seed: &str, draw_id: i64) -> Outcome {
  let mut stream = Stream::new(seed, draw_id);

  match spec {
    Spec::Dice {
      sides,
      count,
      modifier,
    } => {
      let faces: Vec<i64> = (0..*count).map(|_| 1 + stream.below(*sides)).collect();
      let total = faces.iter().sum::<i64>() + modifier;

      Outcome::Dice { faces, total }
    }
    Spec::Range { min, max } => Outcome::Range {
      value: min + stream.below(max - min + 1),
    },
    Spec::Pick { items, take } => Outcome::Pick {
      picked: stream.shuffled(items).into_iter().take(*take as usize).collect(),
    },
    Spec::Shuffle { items } => Outcome::Shuffle {
      order: stream.shuffled(items),
    },
  }
}

/// What was asked, in the words an agent reads it: the shape and what it reads.
pub fn spec_text(spec: &Spec) -> String {
  match spec {
    Spec::Dice {
      sides,
      count,
      modifier,
    } => {
      let modifier = match modifier.signum() {
        0 => String::new(),
        1 => format!(" + {modifier}"),
        _ => format!(" - {}", -modifier),
      };

      format!("{count} die of {sides} faces{modifier}")
    }
    Spec::Range { min, max } => format!("one integer from {min} to {max}"),
    Spec::Pick { items, take } => format!("{} of {} items", take, items.len()),
    Spec::Shuffle { items } => format!("{} items in a fair order", items.len()),
  }
}

/// What came out, in the words an agent reads it.
pub fn result_text(outcome: &Outcome) -> String {
  match outcome {
    Outcome::Dice { faces, total } if faces.len() == 1 && faces[0] == *total => total.to_string(),
    Outcome::Dice { faces, total } => {
      let faces: Vec<String> = faces.iter().map(i64::to_string).collect();

      format!("{} total {total}", faces.join(", "))
    }
    Outcome::Range { value } => value.to_string(),
    Outcome::Pick { picked } => picked.join(", "),
    Outcome::Shuffle { order } => order
      .iter()
      .enumerate()
      .map(|(at, item)| format!("{}. {item}", at + 1))
      .collect::<Vec<_>>()
      .join(" / "),
  }
}

/// The room line a draw posts, as its fields: the label, what was asked, what came out, the number and the
/// seed, the method, the commitment this seed answers and the commitment to the room's next seed. The line
/// itself is a fixed line, rendered in the room's language by the web unit.
pub fn line_of(
  request: &Request,
  outcome: &Outcome,
  draw_id: i64,
  seed: &str,
  answered: Option<String>,
  next: &str,
) -> FixedLine {
  FixedLine::Draw(DrawLine {
    label: request.label.clone(),
    asked: asked_of(&request.spec),
    result: result_of(outcome),
    number: draw_id,
    seed: seed.to_owned(),
    method: DRAW_METHOD.to_owned(),
    answered,
    next: next.to_owned(),
  })
}

/// What was asked, as the room line's code carries it.
fn asked_of(spec: &Spec) -> DrawAsked {
  match spec {
    Spec::Dice {
      sides,
      count,
      modifier,
    } => DrawAsked::Dice {
      count: *count,
      sides: *sides,
      modifier: *modifier,
    },
    Spec::Range { min, max } => DrawAsked::Range {
      min: *min,
      max: *max,
    },
    Spec::Pick { items, take } => DrawAsked::Pick {
      items: items.len(),
      take: *take,
    },
    Spec::Shuffle { items } => DrawAsked::Shuffle { items: items.len() },
  }
}

/// What came out, as the room line's code carries it.
fn result_of(outcome: &Outcome) -> DrawResult {
  match outcome {
    Outcome::Dice { faces, total } => DrawResult::Dice {
      faces: faces.clone(),
      total: *total,
    },
    Outcome::Range { value } => DrawResult::Range { value: *value },
    Outcome::Pick { picked } => DrawResult::Pick {
      picked: picked.clone(),
    },
    Outcome::Shuffle { order } => DrawResult::Shuffle {
      order: order.clone(),
    },
  }
}

/// What a published or recorded draw claims, to be held against the method.
pub struct Claim {
  pub seed: String,
  pub draw_id: i64,
  pub spec: Map<String, Value>,
  pub method: Option<String>,
  pub commitment: Option<String>,
  pub result: Option<Claimed>,
}

/// The result claimed: the record's own value with the text it is kept as, or the text after the result
/// mark on the room line.
pub enum Claimed {
  Recorded { value: Value, text: String },
  Text(String),
}

/// A claim drawn again: the draw, or none where it could not be drawn; each way the claim and the method
/// disagree; and how many of the claim's parts were compared.
pub struct Check {
  pub redrawn: Option<Outcome>,
  pub differences: Vec<String>,
  pub compared: usize,
}

/// A claim drawn again, and where it and the method disagree.
pub fn check_draw(claim: &Claim) -> Check {
  let method = claim.method.as_deref().unwrap_or(DRAW_METHOD);
  let failed = |why: String| Check {
    redrawn: None,
    differences: vec![why],
    compared: 0,
  };

  if method != DRAW_METHOD {
    return failed(format!("the draw names method {method}, and only {DRAW_METHOD} can be checked here"));
  }

  if !is_seed(&claim.seed) {
    return failed("the seed is not 64 lowercase hexadecimal characters".to_owned());
  }

  let request = match read_request(&claim.spec) {
    Ok(request) => request,
    Err(why) => return failed(format!("the request cannot be drawn: {why}")),
  };
  let redrawn = draw_with(&request.spec, &claim.seed, claim.draw_id);
  let mut differences = vec![];
  let mut compared = 0;

  if let Some(commitment) = &claim.commitment {
    compared += 1;

    let hashed = commitment_of(&claim.seed);

    if &hashed != commitment {
      differences.push(format!("the seed hashes to {hashed}, not to the commitment {commitment}"));
    }
  }

  if let Some(claimed) = &claim.result {
    compared += 1;

    let (agrees, shown) = match claimed {
      Claimed::Text(text) => (text.trim() == result_text(&redrawn), text.trim().to_owned()),
      Claimed::Recorded { value, text } => {
        (serde_json::to_value(&redrawn).ok().as_ref() == Some(value), text.clone())
      }
    };

    if !agrees {
      differences.push(format!("the method draws {}, not {shown}", result_text(&redrawn)));
    }
  }

  Check {
    redrawn: Some(redrawn),
    differences,
    compared,
  }
}

fn verdict_lines(differences: &[String]) -> Vec<String> {
  if differences.is_empty() {
    vec!["MATCH".to_owned()]
  } else {
    std::iter::once("DIFFERS".to_owned())
      .chain(differences.iter().map(|difference| format!("- {difference}")))
      .collect()
  }
}

/// One draw as its row keeps it.
pub struct Record {
  pub id: i64,
  pub chat_id: i64,
  pub spec: Value,
  pub label: Option<String>,
  pub commitment: Option<String>,
  pub seed: String,
  pub result: String,
  pub next_commitment: String,
  pub method: String,
}

const COLUMNS: &str =
  r#""id", "chat_id", "spec", "label", "commitment", "seed", "result", "next_commitment", "method""#;

fn record_of(row: &sea_orm::QueryResult) -> Result<Record, sea_orm::DbErr> {
  let spec: String = row.try_get("", "spec")?;

  Ok(Record {
    id: row.try_get("", "id")?,
    chat_id: row.try_get("", "chat_id")?,
    spec: serde_json::from_str(&spec).unwrap_or(Value::Null),
    label: row.try_get("", "label")?,
    commitment: row.try_get("", "commitment")?,
    seed: row.try_get("", "seed")?,
    result: row.try_get("", "result")?,
    next_commitment: row.try_get("", "next_commitment")?,
    method: row.try_get("", "method")?,
  })
}

/// One draw by its number.
pub async fn draw_by_id(db: &DatabaseConnection, id: i64) -> Result<Option<Record>, Fault> {
  let row =
    one(db, &format!(r#"SELECT {COLUMNS} FROM "random_draws" WHERE "id" = $1"#), vec![id.into()]).await?;

  Ok(row.as_ref().map(record_of).transpose()?)
}

// The room's draw before this one, whose next commitment this one's seed answers.
async fn previous_draw(db: &DatabaseConnection, record: &Record) -> Result<Option<Record>, Fault> {
  let row = one(
    db,
    &format!(r#"SELECT {COLUMNS} FROM "random_draws" WHERE "chat_id" = $1 AND "id" < $2 ORDER BY "id" DESC LIMIT 1"#),
    vec![record.chat_id.into(), record.id.into()],
  )
  .await?;

  Ok(row.as_ref().map(record_of).transpose()?)
}

/// What was asked, in the room's words.
pub struct NewDraw<'a> {
  pub chat_id: i64,
  pub via: Option<&'a str>,
  pub message_id: Option<i64>,
  pub mode: &'a str,
  pub request: &'a Request,
}

/// One draw made in one write: the room's pending seed taken (made on its first draw), the draw recorded under
/// its number and drawn there, and the room committed to a fresh seed, so two draws in a room never consume
/// the same seed.
pub async fn make_draw(db: &DatabaseConnection, draw: &NewDraw<'_>) -> Result<(Record, Outcome), Fault> {
  let tx = db.begin().await.map_err(Fault::from)?;
  let pending = one(
    &tx,
    r#"SELECT "next_seed", "next_commitment" FROM "random_draw_rooms" WHERE "chat_id" = $1"#,
    vec![draw.chat_id.into()],
  )
  .await?;
  let (seed, answered): (String, Option<String>) = match pending {
    Some(row) => (row.try_get("", "next_seed")?, Some(row.try_get("", "next_commitment")?)),
    None => (fresh_seed(), None),
  };
  let next = fresh_seed();
  let next_commitment = commitment_of(&next);
  let spec = serde_json::to_string(&draw.request.spec).map_err(|why| Fault::new(why.to_string()))?;
  let written = one(
    &tx,
    r#"INSERT INTO "random_draws" ("chat_id", "via", "message_id", "mode", "shape", "spec", "label", "commitment", "seed",
                                   "result", "next_commitment", "method", "created_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'null', $10, $11, $12) RETURNING "id""#,
    vec![
      draw.chat_id.into(),
      draw.via.map(str::to_owned).into(),
      draw.message_id.into(),
      draw.mode.into(),
      draw.request.spec.shape().into(),
      spec.into(),
      draw.request.label.clone().into(),
      answered.clone().into(),
      seed.clone().into(),
      next_commitment.clone().into(),
      DRAW_METHOD.into(),
      (now_ms() / 1000).into(),
    ],
  )
  .await?;
  let Some(written) = written else {
    return Err(Fault::new("the draw's row was not written"));
  };
  let id = written.try_get::<i64>("", "id")?;
  // The number is the row's own, so the result is drawn once the row has one.
  let outcome = draw_with(&draw.request.spec, &seed, id);
  let result = serde_json::to_string(&outcome).map_err(|why| Fault::new(why.to_string()))?;

  run(
    &tx,
    r#"UPDATE "random_draws" SET "result" = $1 WHERE "id" = $2"#,
    vec![result.into(), id.into()],
  )
  .await?;
  run(
    &tx,
    r#"INSERT INTO "random_draw_rooms" ("chat_id", "next_seed", "next_commitment") VALUES ($1, $2, $3)
       ON CONFLICT ("chat_id") DO UPDATE SET "next_seed" = excluded."next_seed", "next_commitment" = excluded."next_commitment""#,
    vec![draw.chat_id.into(), next.into(), next_commitment.into()],
  )
  .await?;
  tx.commit().await.map_err(Fault::from)?;

  let record = draw_by_id(db, id).await?.ok_or_else(|| Fault::new("the draw's row is gone"))?;

  Ok((record, outcome))
}

/// What the next turn is told of an async draw, as a quiet instruction to the room that asked.
pub fn draw_instruction(request: &Request, record: &Record, outcome: &Outcome) -> String {
  [
    format!(
      "The draw you called in this room has been made: number {}{}.",
      record.id,
      request.label.as_ref().map_or_else(String::new, |label| format!(", for \"{label}\"")),
    ),
    format!("Asked: {}", spec_text(&request.spec)),
    format!("Result: {}", result_text(outcome)),
    format!("Seed: {} (method {DRAW_METHOD})", record.seed),
    "The tool has already posted the line with this result, the seed and the commitments in the room. Carry on with the result in your own voice, with whatever the draw was deciding. Do not post the seed line again, and do not draw again for the same thing.".to_owned(),
    format!(
      "Whoever asks whether it was fair can be told it is drawn again and checked with bmo draw verify {}, or from the room line alone as docs/random-draw.md says.",
      record.id,
    ),
  ]
  .join("\n")
}

/// A draw made in the room, its line posted, answered as `mode` asks: `sync` with the result, `async` with the
/// draw's number and commitment alone, the result queued for the next turn.
pub async fn draw(
  sends: &Sends,
  chat_id: i64,
  mode: Option<&str>,
  args: &Map<String, Value>,
  via: Option<&str>,
) -> Result<Vec<String>, VerbError> {
  let Some(mode) = mode.filter(|mode| MODES.contains(mode)) else {
    return Err(VerbError::Refused(format!("mode must be one of {}", MODES.join(", "))));
  };
  let request = read_request(args).map_err(VerbError::Refused)?;
  // The road is asked before anything is drawn, so a room that cannot carry the line spends no seed.
  let road = sends.road(chat_id, "text").await?;
  let new = NewDraw {
    chat_id,
    via,
    message_id: None,
    mode,
    request: &request,
  };
  let (record, outcome) = make_draw(&sends.db(), &new).await.map_err(store)?;
  let line = sends
    .speech()
    .rendered(&line_of(
      &request,
      &outcome,
      record.id,
      &record.seed,
      record.commitment.clone(),
      &record.next_commitment,
    ))
    .await;

  let sent = sends
    .send(
      &road,
      &ChatSendRequest::new(
        chat_id,
        ChatSend::Text {
          text: line.clone(),
          silent: false,
          fold: false,
        },
      ),
    )
    .await;

  if let Err(NotSent::Refused(why) | NotSent::Failed(why)) = sent {
    return Err(VerbError::Refused(format!(
      "{line}\ndraw {} is recorded, but its line did not go out: {why}. It stands as drawn and is not to be drawn again; the line it would have posted is above",
      record.id,
    )));
  }

  let answered = record.commitment.as_ref().map_or_else(
    || "it is the room's first draw, with no earlier commitment".to_owned(),
    |commitment| format!("its seed answers commitment {commitment}"),
  );

  if mode == "sync" {
    return Ok(vec![
      format!("draw {}: {}", record.id, result_text(&outcome)),
      format!("asked: {}{}", spec_text(&request.spec), request.label.as_ref().map_or_else(String::new, |label| format!(" · {label}"))),
      format!("seed {} · method {} · {answered}", record.seed, record.method),
      "The line with the result and the seed is in the room already; state the result without repeating the seed.".to_owned(),
    ]);
  }

  let agent = match sends.turn_agent() {
    Some(agent) => agent.to_owned(),
    None => wake::agents::voice_of(&sends.receiver(), chat_id)
      .await
      .unwrap_or(sends.receiver().deployment().agent.clone()),
  };
  let queued = Queued {
    sort: Some("random_draw"),
    via,
    ..Queued::default()
  };

  queue(&sends.db(), &agent, chat_id, &draw_instruction(&request, &record, &outcome), &queued)
    .await
    .map_err(store)?;

  Ok(vec![
    format!("draw {} is made and its line is in the room; {answered}.", record.id),
    "The result is not handed back here: it reaches you as a quiet instruction that opens your next turn. End this turn without stating a result.".to_owned(),
  ])
}

/// A verdict: `MATCH` where every part compared agrees, the differences otherwise.
pub struct Verdict {
  pub matched: bool,
  pub lines: Vec<String>,
}

/// A recorded draw checked against the method and against its room's chain: its seed drawn again under its
/// number, hashed against the commitment it answers, and that commitment held to the one the room's
/// previous draw published. Only reads are asked of the archive.
pub async fn verify_recorded(db: &DatabaseConnection, draw_id: i64) -> Result<Verdict, VerbError> {
  let record = draw_by_id(db, draw_id)
    .await?
    .ok_or_else(|| VerbError::Refused(format!("no draw {draw_id} in the archive")))?;
  let previous = previous_draw(db, &record).await?;
  let published = previous.as_ref().map(|previous| previous.next_commitment.clone());
  let recorded = serde_json::from_str(&record.result).unwrap_or(Value::Null);
  let check = check_draw(&Claim {
    seed: record.seed.clone(),
    draw_id: record.id,
    spec: record.spec.as_object().cloned().unwrap_or_default(),
    method: Some(record.method.clone()),
    commitment: record.commitment.clone(),
    result: Some(Claimed::Recorded {
      value: recorded,
      text: record.result.clone(),
    }),
  });
  let mut differences = check.differences;

  if record.commitment != published {
    differences.push(match (&published, &previous) {
      (None, _) | (_, None) => format!(
        "the draw answers commitment {}, but no earlier draw in its room published one",
        record.commitment.as_deref().unwrap_or("null"),
      ),
      (Some(published), Some(previous)) => format!(
        "the draw answers {}, but the room's previous draw {} published {published}",
        record.commitment.as_deref().unwrap_or("no commitment"),
        previous.id,
      ),
    });
  }

  let mut lines = vec![format!(
    "draw {} · chat {} · {}{}",
    record.id,
    record.chat_id,
    record.method,
    record.label.as_ref().map_or_else(String::new, |label| format!(" · {label}")),
  )];

  lines.push(format!("seed {}", record.seed));

  if let Some(redrawn) = &check.redrawn {
    lines.push(format!("drawn again: {}", result_text(redrawn)));
  }

  lines.push(if published.is_none() && record.commitment.is_none() {
    "the room's first draw: no earlier commitment to answer".to_owned()
  } else {
    format!("answers commitment {}", record.commitment.as_deref().unwrap_or("none"))
  });
  lines.extend(verdict_lines(&differences));

  Ok(Verdict {
    matched: differences.is_empty(),
    lines,
  })
}

/// A draw checked from what its room line published alone, with no archive.
pub fn verify_published(claim: &Claim) -> Verdict {
  let check = check_draw(claim);
  let redrawn = check.redrawn.as_ref().map(|redrawn| format!("drawn again: {}", result_text(redrawn)));

  if redrawn.is_some() && check.compared == 0 {
    return Verdict {
      matched: false,
      lines: redrawn
        .into_iter()
        .chain(std::iter::once(
          "nothing given to compare it with: name --commitment, --result or both".to_owned(),
        ))
        .collect(),
    };
  }

  Verdict {
    matched: check.differences.is_empty(),
    lines: redrawn.into_iter().chain(verdict_lines(&check.differences)).collect(),
  }
}

// A whole number as the verb reads one: digits alone, with a sign where it has one.
pub fn whole(text: Option<&str>) -> Option<i64> {
  let text = text?;
  let digits = text.strip_prefix('-').unwrap_or(text);

  if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
    return None;
  }

  text.parse().ok().filter(|number: &i64| (*number as f64).abs() <= SAFE_INTEGER)
}

// The request, as the tool's own fields in one JSON object.
pub fn spec_argument(spec: Option<&str>) -> Result<Map<String, Value>, VerbError> {
  let text = spec.ok_or_else(|| VerbError::Refused(USAGE.to_owned()))?;

  match serde_json::from_str(text) {
    Ok(Value::Object(spec)) => Ok(spec),
    _ => Err(VerbError::Refused(
      "--spec must be one JSON object, such as '{\"shape\":\"dice\",\"sides\":6}'".to_owned(),
    )),
  }
}

// A verdict as the verb answers it: its lines, ending in failure where the draw and the method disagree.
pub fn judged(verdict: Verdict) -> Result<Vec<String>, VerbError> {
  if verdict.matched {
    Ok(verdict.lines)
  } else {
    Err(VerbError::Partly {
      lines: verdict.lines,
      refusal: String::new(),
    })
  }
}

pub const USAGE: &str = "usage:
  bmo draw draw --chat <chatId> --mode sync|async --spec '<json>' [--via <road>]
  bmo draw verify <drawId>
  bmo draw verify --seed <hex> --draw <drawId> --spec '<json>' [--commitment <hex>] [--result '<result text>']";

// The store failed while answering.
fn store(why: Fault) -> VerbError {
  VerbError::Store(why)
}

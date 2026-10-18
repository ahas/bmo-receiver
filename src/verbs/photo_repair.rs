//! The picture put back under the rows of the receiver's own sends that kept none (docs/pictures.md, 「Pictures
//! sent」). Such a row says the photo mark and then the address the picture was sent by, and nothing else, so a
//! screen reading one finds a line of text where the picture should be; where the address can still be reached,
//! the picture is filed under its message again.
//!
//! Only the picture is put back. The row's text is left as it stands, since the address in it is the only record
//! of where that picture came from.

use std::path::{Path, PathBuf};

use receiver_policy::roads::ROADS;

use reqwest::Client;

use crate::data_root::DataRoot;
use crate::receiver::{Receiver, pictures};
use crate::verbs::VerbError;
use crate::{Fault, all, run};

// The quality a repaired picture is redrawn at before it is filed.
const REDRAW_QUALITY: u8 = 90;

/// One row walked: where it is, the address it was sent by, and where its picture was filed — none where
/// nothing was written, or where the address could not be reached or was no picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureRepair {
  pub chat_id: i64,
  pub message_id: i64,
  pub address: String,
  pub kept_at: Option<String>,
}

// The mark a received or sent picture's row opens with, as the roads write it.
fn photo_mark() -> String {
  ROADS.marks.picture.replace("{caption}", "")
}

/// The address a broken picture row was sent by: its last line, with the photo mark taken off where the
/// row is the mark and the address alone. A web address counts, and a file only where it sits under the
/// data root, which is where the receiver puts what it gathers; none for any other row.
pub fn sent_picture_address(data_root: &DataRoot, text: &str) -> Option<String> {
  let lines: Vec<&str> = text.split('\n').collect();
  let last = lines.last().copied().unwrap_or_default().trim();
  let mark = photo_mark();
  let address = match last.strip_prefix(mark.as_str()) {
    Some(rest) if lines.len() == 1 => rest.trim(),
    _ => last,
  };

  if address.starts_with("http://") || address.starts_with("https://") {
    return Some(address.to_owned());
  }

  let path = Path::new(address);

  (path.is_absolute() && lexical(path).starts_with(lexical(data_root.path()))).then(|| address.to_owned())
}

// A path made absolute without following links, so a file under the data root counts wherever a link stands.
fn lexical(path: &Path) -> PathBuf {
  if path.is_absolute() {
    return path.to_path_buf();
  }

  std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

// The picture behind an address redrawn as JPEG laid on white and filed under its message; none where it
// could not be reached or was no picture.
async fn filed(data_root: &DataRoot, chat_id: i64, message_id: i64, address: &str) -> Option<String> {
  let bytes = fetched(address).await.ok()?;
  let picture = pictures::decode(&bytes).ok()?;
  let laid = pictures::on_white(&picture);
  let redrawn = pictures::jpeg(&laid, REDRAW_QUALITY).ok()?;
  let place = data_root.images().join(chat_id.to_string()).join(format!("{message_id}.jpg"));

  std::fs::create_dir_all(place.parent().unwrap_or(&place)).ok()?;
  std::fs::write(&place, redrawn).ok()?;

  Some(place.to_string_lossy().into_owned())
}

/// The bytes behind an address: a web address fetched, a file of the data root read.
pub async fn fetched(address: &str) -> Result<Vec<u8>, String> {
  if address.starts_with("http://") || address.starts_with("https://") {
    let answer = Client::new().get(address).send().await;
    let bytes = match answer {
      Ok(answer) => answer.bytes().await,
      Err(why) => Err(why),
    };

    Ok(bytes.map(|held| held.to_vec()).unwrap_or_default())
  } else {
    Ok(std::fs::read(address).unwrap_or_default())
  }
}

/// Every such row walked once. Without `write` nothing is downloaded and nothing written: the answer is
/// which rows carry an address worth trying.
pub async fn repair_sent_pictures(
  receiver: &Receiver,
  data_root: &DataRoot,
  write: bool,
) -> Result<Vec<PictureRepair>, VerbError> {
  let rows = all(
    receiver.db(),
    r#"SELECT "chat_id", "message_id", "text" FROM "messages"
       WHERE "sender_name" = $1 AND "image_path" IS NULL AND "text" LIKE $2
       ORDER BY "sent_at""#,
    vec![receiver.deployment().sender_name.clone().into(), format!("{}%", photo_mark()).into()],
  )
  .await?;
  let mut repairs = vec![];

  for row in rows {
    let chat_id: i64 = row.try_get("", "chat_id").map_err(Fault::from)?;
    let message_id: i64 = row.try_get("", "message_id").map_err(Fault::from)?;
    let text: String = row.try_get("", "text").map_err(Fault::from)?;
    let Some(address) = sent_picture_address(data_root, &text) else {
      continue;
    };
    let kept_at = if write { filed(data_root, chat_id, message_id, &address).await } else { None };

    if let Some(path) = &kept_at {
      run(
        receiver.db(),
        r#"UPDATE "messages" SET "image_path" = $1 WHERE "chat_id" = $2 AND "message_id" = $3"#,
        vec![path.clone().into(), chat_id.into(), message_id.into()],
      )
      .await?;
    }

    repairs.push(PictureRepair {
      chat_id,
      message_id,
      address,
      kept_at,
    });
  }

  Ok(repairs)
}

/// The lines `bmo archive photo-repair` prints: each row tried, and how many carry an address or now carry
/// their picture.
pub async fn photo_repair(
  receiver: &Receiver,
  data_root: &DataRoot,
  write: bool,
) -> Result<Vec<String>, VerbError> {
  let repairs = repair_sent_pictures(receiver, data_root, write).await?;
  let mut lines: Vec<String> = repairs
    .iter()
    .map(|repair| {
      let outcome = match (&repair.kept_at, write) {
        (_, false) => "would be tried",
        (Some(kept_at), true) => kept_at.as_str(),
        (None, true) => "could not be reached",
      };

      format!("{}/{}  {}  {outcome}", repair.chat_id, repair.message_id, repair.address)
    })
    .collect();
  let kept = repairs.iter().filter(|repair| repair.kept_at.is_some()).count();

  lines.push(if write {
    format!("{kept} of {} rows now carry their picture", repairs.len())
  } else {
    format!("{} rows carry an address; run again with --write", repairs.len())
  });

  Ok(lines)
}

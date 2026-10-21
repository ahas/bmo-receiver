//! The old archive's move into the receiver's store (src/import.rs, TODO #1923): the mapping, the
//! stale-root rewrite, the refusal of a store already holding rows, and the count-and-sample compare,
//! over a small export written into a temporary folder and a scratch database of the test's own.

use std::fs;
use std::path::PathBuf;

use bmo_receiver::import::{self, Cell};
use serde_json::{Value, json};

use crate::common::Scratch;

/// A small export in the shape the item's export hand writes: every table the import knows, with the
/// rows given and none elsewhere. The stale prefix and the live root stand under the test's own
/// temporary folder, so no deployment's path is a fixture.
fn fixture() -> (tempfile::TempDir, PathBuf, import::Roots) {
  let dir = tempfile::tempdir().expect("a temporary folder");
  let roots =
    import::Roots::new(dir.path().join("old-root").to_string_lossy(), dir.path().join("live-root"));
  let stale = roots.stale.clone();
  let rows: Vec<(&str, Vec<Value>, Vec<&str>)> = vec![
    (
      "messages",
      vec![
        json!({"chat_id": 1, "message_id": 10, "sender_id": 7, "sender_name": "BMO", "sent_at": 100, "text": "hello", "image_path": format!("{stale}/images/1/10.jpg"), "reply_to_message_id": null, "answer_seconds": null, "round": null, "album": null, "via": "record"}),
        json!({"chat_id": 1, "message_id": 11, "sender_id": 7, "sender_name": "BMO", "sent_at": 200, "text": "two", "image_path": null, "reply_to_message_id": null, "answer_seconds": null, "round": null, "album": null, "via": "record"}),
      ],
      vec!["chat_id", "message_id"],
    ),
    (
      "collected_items",
      vec![
        json!({"id": 1, "chat_id": 1, "message_id": 10, "sender_name": "BMO", "kind": "audio", "source": "bmo:voice", "stored_path": format!("{stale}/collected/1/a.ogg"), "summary": null, "collected_at": 150, "file_unique_id": null}),
      ],
      vec!["id"],
    ),
    ("settings", vec![json!({"key": "mode", "value": "normal"})], vec!["key"]),
    (
      "telegram_messages",
      vec![json!({"chat_id": 1, "message_id": 10, "metadata": "{}"})],
      vec!["chat_id", "message_id"],
    ),
  ];

  let mut manifest = Vec::new();
  for table in import::TABLES {
    let (held, key): (Vec<Value>, Vec<&str>) = rows
      .iter()
      .find(|(source, _, _)| *source == table.source)
      .map(|(_, rows, key)| (rows.clone(), key.clone()))
      .unwrap_or_else(|| (Vec::new(), vec![table.columns[0]]));
    let file = format!("{}.ndjson", table.source);
    let mut body = String::new();
    for row in &held {
      body.push_str(&row.to_string());
      body.push('\n');
    }
    fs::write(dir.path().join(&file), body).expect("the export file");
    manifest.push(json!({
      "source": table.source,
      "file": file,
      "count": held.len(),
      "columns": [],
      "key": key
    }));
  }
  fs::write(
    dir.path().join("manifest.json"),
    json!({
      "unit": "receiver",
      "snapshot": "1:1:",
      "readAt": "2026-10-21 17:00:00+09",
      "readOnly": true,
      "tables": manifest
    })
    .to_string(),
  )
  .expect("the manifest");

  let path = dir.path().to_path_buf();
  (dir, path, roots)
}

#[test]
fn the_stale_root_is_rewritten_and_every_other_path_stands() {
  let roots = import::Roots::new("/archive/old-root", "/data/live-root");

  assert_eq!(roots.live_path("/archive/old-root/images/1/10.jpg"), "/data/live-root/images/1/10.jpg");
  assert_eq!(roots.live_path("images/1/10.jpg"), "images/1/10.jpg");
  assert_eq!(import::Roots::new("", "/data/live-root").live_path("/archive/a.jpg"), "/archive/a.jpg");
}

#[test]
fn a_time_is_written_in_milliseconds_and_a_number_path_column_keeps_its_text() {
  let table = import::TABLES.iter().find(|table| table.source == "messages").expect("messages");
  let roots = import::Roots::new("/archive/old-root", "/data/live-root");
  let row = import::row(
    table,
    &json!({"chat_id": 1, "message_id": 10, "sender_id": 7, "sender_name": "BMO", "sent_at": 100, "text": "hello", "image_path": null, "reply_to_message_id": null, "answer_seconds": null, "round": null, "album": null, "via": "record"}),
    &roots,
  );
  let get = |column: &str| {
    row
      .iter()
      .find(|(held, _)| *held == column)
      .map(|(_, cell)| cell.clone())
      .expect("a mapped column")
  };
  assert_eq!(get("sent_at"), Cell::Int(100_000));
  assert_eq!(get("sender_name"), Cell::Text("BMO".to_owned()));
  assert_eq!(get("image_path"), Cell::Null);
}

#[test]
fn a_morning_edition_keeps_its_time_and_a_holiday_keeps_its_day() {
  let roots = import::Roots::new("", "/data/live-root");
  let cell = |row: &[(&'static str, Cell)], column: &str| {
    row
      .iter()
      .find(|(held, _)| *held == column)
      .map(|(_, cell)| cell.clone())
      .expect("a mapped column")
  };

  let brief = import::TABLES.iter().find(|table| table.source == "news_briefs").expect("news_briefs");
  let row = import::row(
    brief,
    &json!({"id": 3, "chat_id": -7, "send_at": "08:30", "lead_minutes": 40, "share_hours": 24, "enabled": 1, "last_day": "2026-10-21", "created_at": 1_700_000_000}),
    &roots,
  );
  assert_eq!(cell(&row, "created_at"), Cell::Int(1_700_000_000_000));
  assert_eq!(cell(&row, "send_at"), Cell::Text("08:30".to_owned()));
  assert_eq!(cell(&row, "enabled"), Cell::Int(1));
  assert_eq!(cell(&row, "last_day"), Cell::Text("2026-10-21".to_owned()));

  let holidays = import::TABLES.iter().find(|table| table.source == "holidays").expect("holidays");
  let row = import::row(holidays, &json!({"day": "2026-10-03", "name": "day off"}), &roots);
  assert_eq!(cell(&row, "day"), Cell::Text("2026-10-03".to_owned()));
  assert_eq!(cell(&row, "name"), Cell::Text("day off".to_owned()));
}

#[tokio::test]
async fn the_import_writes_its_rows_refuses_a_second_run_and_the_compare_finds_a_changed_row() {
  let (_folder, export, roots) = fixture();
  let scratch = Scratch::new().await;
  let address = scratch.address();

  let report = import::run(&address, &export, false, &roots).await.expect("the import");
  assert_eq!(report.snapshot, "1:1:");
  assert!(report.sample.failures.is_empty(), "{:?}", report.sample.failures);

  let image = scratch
    .texts("SELECT image_path FROM messages WHERE chat_id = 1 AND message_id = 10")
    .await;
  let wanted = roots.live.join("images/1/10.jpg");
  assert_eq!(image.first().map(String::as_str), Some(wanted.to_string_lossy().as_ref()));
  assert_eq!(scratch.count("SELECT COUNT(*) FROM message_metadata", vec![]).await, 1);

  let again = import::run(&address, &export, false, &roots).await;
  assert!(again.is_err(), "a store holding rows is refused");
  assert!(again.unwrap_err().contains("already holds rows"));

  let passes = import::verify(&address, &export, &roots).await.expect("the compare");
  assert!(passes.failures.is_empty(), "{:?}", passes.failures);

  scratch
    .run("UPDATE messages SET text = 'changed' WHERE chat_id = 1 AND message_id = 10", vec![])
    .await;
  let changed = import::verify(&address, &export, &roots).await.expect("the compare");
  assert!(
    changed.failures.iter().any(|failure| failure.contains("text")),
    "{:?}",
    changed.failures
  );
}

#[tokio::test]
async fn a_dry_run_writes_nothing() {
  let (_folder, export, roots) = fixture();
  let scratch = Scratch::new().await;
  let address = scratch.address();

  let report = import::run(&address, &export, true, &roots).await.expect("the dry run");
  assert!(report.dry_run);

  assert_eq!(scratch.count("SELECT COUNT(*) FROM messages", vec![]).await, 0);
}

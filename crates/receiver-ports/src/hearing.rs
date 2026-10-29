//! The listening tool, bmo-ear (`ear/listen`), as the receiver calls it through the tool host's control
//! socket (docs/boundary-api.md, 「The tool host」; bmo-pocket's `docs/api.md`, 「The control socket」): one saved
//! recording answered with what was heard, how long it ran and the language it was heard in.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::future::BoxFuture;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::http::{Answer, Caller, Peer, PortFailure, WireError};

/// What bmo-ear heard in one recording.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
  pub transcript: String,
  pub language: String,
  /// How long the recording ran, where the tool read it.
  pub seconds: Option<f64>,
}

/// The receiver's way to hear a saved recording: bmo-ear, or a stand-in where a test runs one.
pub trait Hearing: Send + Sync {
  fn transcribe<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Answer<Heard>>;
}

#[derive(Deserialize)]
struct SessionAnswer {
  token: String,
}

#[derive(Deserialize)]
struct CallAnswer {
  result: Value,
}

/// bmo-ear on the tool host's control socket, with a tool session the receiver issues for itself and keeps.
pub struct HttpHearing {
  socket: PathBuf,
  data_root: PathBuf,
  issuer: Caller,
  session: Arc<Mutex<Option<String>>>,
}

impl HttpHearing {
  /// The listener on the host at `socket`, reached with the issuer token in `token_file`, its calls naming the
  /// receiver's `data_root` for the tool.
  pub fn new(socket: PathBuf, token_file: PathBuf, data_root: PathBuf) -> Self {
    let token = std::fs::read_to_string(&token_file).unwrap_or_default().trim().to_owned();
    let issuer = Caller::over_socket(
      &socket,
      &Peer {
        base: "http://tool-host".to_owned(),
        token,
      },
    );

    Self {
      socket,
      data_root,
      issuer,
      session: Arc::new(Mutex::new(None)),
    }
  }

  // The session's token, issued once and kept; a token the host no longer answers with is issued again.
  async fn session(&self) -> Answer<String> {
    let mut held = self.session.lock().await;

    if let Some(token) = held.as_ref() {
      return Ok(token.clone());
    }

    let record = json!({
      "holderName": "bmo-receiver",
      "namespaces": ["bmo"],
      "capabilities": ["room.tools"],
      "grants": [],
      "variables": { "BMO_DATA_DIR": self.data_root.to_string_lossy() },
      "labels": { "line": "chat" },
      "ttlMs": 3_600_000,
    });
    let answer: SessionAnswer = self.issuer.post(&["tool-sessions"], &record).await?;

    *held = Some(answer.token.clone());
    Ok(answer.token)
  }

  async fn call(&self, token: &str, path: &Path) -> Answer<Heard> {
    let caller = Caller::over_socket(
      &self.socket,
      &Peer {
        base: "http://tool-host".to_owned(),
        token: token.to_owned(),
      },
    );
    let answer: CallAnswer = caller
      .post(
        &["tool-calls"],
        &json!({
          "namespace": "ear",
          "name": "listen",
          "input": { "path": path },
        }),
      )
      .await?;
    self.read(answer.result)
  }

  fn read(&self, result: Value) -> Answer<Heard> {
    let failed = result.get("isError").and_then(Value::as_bool) == Some(true);

    if failed {
      let why = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|one| one.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("the tool failed")
        .to_owned();

      return Err(PortFailure::Answered {
        url: "http://tool-host/tool-calls".to_owned(),
        status: 502,
        answer: Some(WireError {
          code: "tool-failed".to_owned(),
          error: why,
        }),
      });
    }

    let said = result
      .get("structuredContent")
      .cloned()
      .filter(|content| content.is_object())
      .ok_or_else(|| undecodable("the call answered no structured transcript"))?;
    let transcript = said
      .get("transcript")
      .and_then(Value::as_str)
      .ok_or_else(|| undecodable("the transcript is not text"))?
      .to_owned();
    let language = said.get("language").and_then(Value::as_str).unwrap_or_default().to_owned();
    let seconds = said.get("duration").and_then(Value::as_f64);

    Ok(Heard {
      transcript,
      language,
      seconds,
    })
  }
}

fn undecodable(why: &str) -> PortFailure {
  PortFailure::Undecodable {
    url: "http://tool-host/tool-calls".to_owned(),
    why: why.to_owned(),
  }
}

impl Hearing for HttpHearing {
  fn transcribe<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Answer<Heard>> {
    Box::pin(async move {
      let token = self.session().await?;
      let first = self.call(&token, path).await;

      match first {
        Err(PortFailure::Answered {
          status: 401 | 403, ..
        }) => {
          *self.session.lock().await = None;
          let token = self.session().await?;

          self.call(&token, path).await
        }
        other => other,
      }
    })
  }
}

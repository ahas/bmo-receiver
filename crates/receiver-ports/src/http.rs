//! How a port reaches its unit: a call of a JSON body, a GET with its query, or a GET held open for the events
//! it streams, at a path under the unit's base address and behind the unit's token, offered again only while the
//! connection is refused (`docs/boundary-api.md`, 「Calls the receiver makes」).

use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Method, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Where a unit answers and the token it is called with, as the receiver's configuration gives them. A unit that
/// takes no token is given none, and its calls carry no `Authorization` header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Peer {
  pub base: String,
  #[serde(default)]
  pub token: String,
}

/// How a wait between two offers of a call is spent. The caller counts the time it has waited by the intervals
/// it asked for, never by reading a clock, so a waiter that returns at once makes a whole window pass at once.
pub trait Wait: Send + Sync {
  fn wait(&self, interval: Duration) -> BoxFuture<'_, ()>;
}

/// Waits on the real clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealClock;

impl Wait for RealClock {
  fn wait(&self, interval: Duration) -> BoxFuture<'_, ()> {
    Box::pin(tokio::time::sleep(interval))
  }
}

/// How long a call whose connection was refused keeps being offered, and how often.
#[derive(Clone)]
pub struct Patience {
  /// The most time spent waiting before the call is given up.
  pub window: Duration,
  /// The wait before each new offer; zero offers a call only once.
  pub every: Duration,
  pub waiter: Arc<dyn Wait>,
}

impl Default for Patience {
  /// Two minutes, offered every two seconds, on the real clock.
  fn default() -> Self {
    Self {
      window: Duration::from_secs(120),
      every: Duration::from_secs(2),
      waiter: Arc::new(RealClock),
    }
  }
}

impl fmt::Debug for Patience {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Patience")
      .field("window", &self.window)
      .field("every", &self.every)
      .finish_non_exhaustive()
  }
}

/// The error shape a unit answers a status that is no success with: its code and its English sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireError {
  pub code: String,
  pub error: String,
}

/// Why a call came to no answer of the call's shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortFailure {
  /// The base address is no `http` address a path can be put under.
  NotAnAddress { base: String },
  /// Nothing listened at the address for the whole patience window.
  Unreachable {
    url: String,
    waited: Duration,
    why: String,
  },
  /// The unit answered, and not with success. Such a call is never offered again.
  Answered {
    url: String,
    status: u16,
    /// The error shape, where the answer carried one.
    answer: Option<WireError>,
  },
  /// The unit answered with success, in a body that is not the call's answer.
  Undecodable { url: String, why: String },
  /// The call failed after it may have reached a listener, so it is not offered again.
  Transport { url: String, why: String },
}

impl PortFailure {
  /// The code the unit refused with, where it answered with its error shape.
  pub fn code(&self) -> Option<&str> {
    match self {
      Self::Answered {
        answer: Some(answer),
        ..
      } => Some(&answer.code),
      _ => None,
    }
  }

  /// The status the unit answered with, where it answered.
  pub fn status(&self) -> Option<u16> {
    match self {
      Self::Answered { status, .. } => Some(*status),
      _ => None,
    }
  }
}

impl fmt::Display for PortFailure {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NotAnAddress { base } => write!(f, "{base} is not an http address a unit can be reached at"),
      Self::Unreachable { url, waited, why } => {
        write!(f, "Nothing was listening at {url} after waiting {waited:?}: {why}")
      }
      Self::Answered {
        url,
        status,
        answer: Some(answer),
      } => write!(f, "{url} answered {status}: {}", answer.error),
      Self::Answered {
        url,
        status,
        answer: None,
      } => write!(f, "{url} answered {status}."),
      Self::Undecodable { url, why } => {
        write!(f, "{url} answered in a shape the call does not have: {why}")
      }
      Self::Transport { url, why } => write!(f, "The call to {url} failed: {why}"),
    }
  }
}

impl std::error::Error for PortFailure {}

/// What a call on a unit comes back with.
pub type Answer<T> = Result<T, PortFailure>;

/// One unit as a port reaches it.
#[derive(Debug, Clone)]
pub struct Caller {
  base: String,
  token: String,
  patience: Patience,
  client: Client,
}

impl Caller {
  /// A caller of the unit at the peer's base address, with the default patience. It reads no environment of its
  /// own, so no proxy is taken from it.
  pub fn new(peer: &Peer) -> Self {
    let client = Client::builder()
      .no_proxy()
      .build()
      .expect("a client with no TLS and no proxy always builds");

    Self {
      base: peer.base.clone(),
      token: peer.token.clone(),
      patience: Patience::default(),
      client,
    }
  }

  /// A caller of a unit that answers on a Unix socket rather than at an address: the peer's base names the path's
  /// scheme and host the requests carry, and every connection is the socket's.
  pub fn over_socket(socket: &std::path::Path, peer: &Peer) -> Self {
    let client = Client::builder()
      .no_proxy()
      .unix_socket(socket)
      .build()
      .expect("a client on a socket with no TLS and no proxy always builds");

    Self {
      base: peer.base.clone(),
      token: peer.token.clone(),
      patience: Patience::default(),
      client,
    }
  }

  pub fn with_patience(mut self, patience: Patience) -> Self {
    self.patience = patience;
    self
  }

  /// The address of the path made of `segments` under the base, each percent-encoded.
  pub fn url(&self, segments: &[&str]) -> Answer<Url> {
    let not_an_address = || PortFailure::NotAnAddress {
      base: self.base.clone(),
    };
    let mut url = Url::parse(&self.base).map_err(|_| not_an_address())?;

    if url.scheme() != "http" {
      return Err(not_an_address());
    }

    url.path_segments_mut().map_err(|()| not_an_address())?.pop_if_empty().extend(segments);

    Ok(url)
  }

  /// A call of `method` with a JSON body, its successful answer decoded as the call's.
  pub async fn call<T: DeserializeOwned>(
    &self,
    method: Method,
    segments: &[&str],
    body: &impl Serialize,
  ) -> Answer<T> {
    let (url, answer) = self.sent(method, segments, Some(body), &[]).await?;

    decoded(url, answer).await
  }

  /// A call of `method` with a JSON body whose success carries nothing the receiver reads.
  pub async fn done(&self, method: Method, segments: &[&str], body: &impl Serialize) -> Answer<()> {
    let (url, answer) = self.sent(method, segments, Some(body), &[]).await?;

    succeeded(url, answer).await
  }

  /// A call of `method` with no body, whose success carries nothing the receiver reads.
  pub async fn without(&self, method: Method, segments: &[&str]) -> Answer<()> {
    let (url, answer) = self.sent::<()>(method, segments, None, &[]).await?;

    succeeded(url, answer).await
  }

  /// A POST with a JSON body, its successful answer decoded as the call's.
  pub async fn post<T: DeserializeOwned>(&self, segments: &[&str], body: &impl Serialize) -> Answer<T> {
    self.call(Method::POST, segments, body).await
  }

  /// A PATCH with a JSON body, its successful answer decoded as the call's.
  pub async fn patch<T: DeserializeOwned>(&self, segments: &[&str], body: &impl Serialize) -> Answer<T> {
    self.call(Method::PATCH, segments, body).await
  }

  /// A POST with a JSON body whose refusal of the body is an answer of its own: success decoded as the call's,
  /// a 400 whose body reads as the unit's refusal handed back as that refusal, and anything else the failure it
  /// is.
  pub async fn checked<T: DeserializeOwned, R: DeserializeOwned>(
    &self,
    segments: &[&str],
    body: &impl Serialize,
  ) -> Answer<Result<T, R>> {
    let (url, answer) = self.sent(Method::POST, segments, Some(body), &[]).await?;

    if answer.status() != reqwest::StatusCode::BAD_REQUEST {
      return decoded(url, answer).await.map(Ok);
    }

    read_refusal(url, answer).await.map(Err)
  }

  /// A DELETE whose refusal of its path is an answer of its own: success carries nothing the receiver reads, a
  /// 400 whose body reads as the unit's refusal handed back as that refusal, and anything else the failure it is.
  pub async fn checked_delete<R: DeserializeOwned>(&self, segments: &[&str]) -> Answer<Result<(), R>> {
    let (url, answer) = self.sent::<()>(Method::DELETE, segments, None, &[]).await?;

    if answer.status().is_success() {
      return Ok(Ok(()));
    }

    if answer.status() != reqwest::StatusCode::BAD_REQUEST {
      return Err(failed(url, answer).await);
    }

    read_refusal(url, answer).await.map(Err)
  }

  /// A GET with the query's fields, its successful answer decoded as the call's.
  pub async fn get<T: DeserializeOwned>(&self, segments: &[&str], query: &[(&str, String)]) -> Answer<T> {
    let (url, answer) = self.sent::<()>(Method::GET, segments, None, query).await?;

    decoded(url, answer).await
  }

  /// A DELETE with the query's fields, its successful answer decoded as the call's.
  pub async fn delete<T: DeserializeOwned>(
    &self,
    segments: &[&str],
    query: &[(&str, String)],
  ) -> Answer<T> {
    let (url, answer) = self.sent::<()>(Method::DELETE, segments, None, query).await?;

    decoded(url, answer).await
  }

  /// A GET with the query's fields, answered `None` where the unit says it found nothing (404).
  pub async fn found<T: DeserializeOwned>(
    &self,
    segments: &[&str],
    query: &[(&str, String)],
  ) -> Answer<Option<T>> {
    let (url, answer) = self.sent::<()>(Method::GET, segments, None, query).await?;

    if answer.status() == reqwest::StatusCode::NOT_FOUND {
      return Ok(None);
    }

    decoded(url, answer).await.map(Some)
  }

  /// A GET with the query's fields whose answer goes on arriving, a stream of server-sent events, answered once
  /// its head says it succeeded.
  pub async fn open(&self, segments: &[&str], query: &[(&str, String)]) -> Answer<reqwest::Response> {
    let (url, answer) = self.sent::<()>(Method::GET, segments, None, query).await?;

    if answer.status().is_success() {
      return Ok(answer);
    }

    Err(failed(url, answer).await)
  }

  // The request sent, offered again only while the connection is refused and the patience window lasts.
  async fn sent<B: Serialize>(
    &self,
    method: Method,
    segments: &[&str],
    body: Option<&B>,
    query: &[(&str, String)],
  ) -> Answer<(String, reqwest::Response)> {
    let url = self.url(segments)?;
    let shown = url.to_string();
    let body = body.map(serde_json::to_vec).transpose().map_err(|error| PortFailure::Transport {
      url: shown.clone(),
      why: error.to_string(),
    })?;
    let request = || {
      let mut request = self.client.request(method.clone(), url.clone());

      if !self.token.is_empty() {
        request = request.bearer_auth(&self.token);
      }

      if !query.is_empty() {
        request = request.query(query);
      }

      match &body {
        Some(body) => request.header(CONTENT_TYPE, "application/json").body(body.clone()),
        None => request,
      }
    };
    let Patience {
      window,
      every,
      waiter,
    } = &self.patience;
    let mut waited = Duration::ZERO;

    loop {
      let error = match request().send().await {
        Ok(answer) => return Ok((shown, answer)),
        Err(error) => error,
      };

      if !refused(&error) {
        return Err(PortFailure::Transport {
          url: shown,
          why: chain(&error),
        });
      }

      if every.is_zero() || waited + *every > *window {
        return Err(PortFailure::Unreachable {
          url: shown,
          waited,
          why: chain(&error),
        });
      }

      waiter.wait(*every).await;
      waited += *every;
    }
  }
}

// A successful answer read as the call's, and any other as the failure it is.
async fn decoded<T: DeserializeOwned>(url: String, answer: reqwest::Response) -> Answer<T> {
  if !answer.status().is_success() {
    return Err(failed(url, answer).await);
  }

  let bytes = answer.bytes().await.map_err(|error| PortFailure::Transport {
    url: url.clone(),
    why: chain(&error),
  })?;

  serde_json::from_slice(&bytes).map_err(|error| PortFailure::Undecodable {
    url,
    why: error.to_string(),
  })
}

// A successful answer whatever it carries, and any other as the failure it is.
async fn succeeded(url: String, answer: reqwest::Response) -> Answer<()> {
  if answer.status().is_success() {
    return Ok(());
  }

  Err(failed(url, answer).await)
}

// A 400's body read as the unit's refusal of a field.
async fn read_refusal<R: DeserializeOwned>(url: String, answer: reqwest::Response) -> Answer<R> {
  let bytes = answer.bytes().await.map_err(|error| PortFailure::Transport {
    url: url.clone(),
    why: chain(&error),
  })?;

  serde_json::from_slice(&bytes).map_err(|_| PortFailure::Answered {
    url,
    status: 400,
    answer: serde_json::from_slice(&bytes).ok(),
  })
}

// An answer that is no success, with the error shape it carried where it carried one.
async fn failed(url: String, answer: reqwest::Response) -> PortFailure {
  let status = answer.status().as_u16();

  match answer.bytes().await {
    Ok(bytes) => PortFailure::Answered {
      url,
      status,
      answer: serde_json::from_slice(&bytes).ok(),
    },
    Err(error) => PortFailure::Transport {
      url,
      why: chain(&error),
    },
  }
}

/// Whether the connection was refused, so nothing listened and the call cannot have been received.
fn refused(error: &reqwest::Error) -> bool {
  let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(error);

  while let Some(error) = cause {
    if error
      .downcast_ref::<io::Error>()
      .is_some_and(|error| error.kind() == io::ErrorKind::ConnectionRefused)
    {
      return true;
    }

    cause = error.source();
  }

  false
}

/// The error and every cause beneath it, in one line.
fn chain(error: &reqwest::Error) -> String {
  let mut words = error.to_string();
  let mut cause = std::error::Error::source(error);

  while let Some(error) = cause {
    words.push_str(": ");
    words.push_str(&error.to_string());
    cause = error.source();
  }

  words
}

/// The answer of a call that says only whether it was done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Done {
  pub ok: bool,
}

/// The body of a call that sends nothing.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Nothing {}

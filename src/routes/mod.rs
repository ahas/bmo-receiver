//! The receiver's own routes (docs/boundary-api.md, 「The receiver's own routes」): what the console, the send
//! tool and the `bmo` command line reach the receiver on. Every one answers only a request carrying the
//! receiver's token or the console relay's, the routes that file a person's words the console relay's alone
//! (docs/api.md, 「Auth」), and a refusal with its code and the English sentence naming it.

mod accounts;
mod avatars;
mod brain;
mod briefing;
mod console;
mod events;
mod figures;
mod file_fills;
mod handoffs;
mod handovers;
mod hooks;
mod instructions;
mod message_acts;
mod message_files;
mod openings;
mod owner_words;
mod presence;
mod room_removal;
mod rooms;
mod say;
mod sends;
mod speakers;
mod stages;
mod supervisors;
mod talks;
mod tones;
mod writable;

use std::sync::Arc;

use axum::{
  Router,
  body::Bytes,
  extract::{DefaultBodyLimit, RawQuery, Request, State},
  http::StatusCode,
  middleware::{Next, from_fn, from_fn_with_state},
  response::{IntoResponse, Json, Response},
  routing::MethodRouter,
  routing::{delete, get, patch, post, put},
};
use headers::authorization::Bearer;
use headers::{Authorization, HeaderMapExt};
use serde_json::json;

use receiver_codes::Refused;
use receiver_ports::http::PortFailure;

use crate::Fault;
use crate::receiver::Receiver;
use crate::routes::owner_words::instruct;
use crate::routes::say::{say, say_file};
use crate::routes::sends::sends;
use crate::routes::talks::talk;

// The readings a route's own tests hold, kept beside the handler that uses them.
pub use crate::routes::events::{TYPES, leading_whole};
pub use crate::routes::openings::{SUMMARY_CHARS, rounds_of, summary_of};

/// What every route stands on: the receiver and the bearers it answers behind.
#[derive(Clone)]
struct Served {
  receiver: Arc<Receiver>,
  token: Arc<str>,
  console: Option<Arc<str>>,
}

/// The bearers the receiver answers behind: its own token, and the console relay's own credential where the
/// configuration names one (docs/boundary-api.md, 「Configuration」).
#[derive(Clone, Debug)]
pub struct Bearers {
  token: String,
  console: Option<String>,
}

impl Bearers {
  /// The receiver's own token alone, under which no route that files a person's words is answered.
  pub fn new(token: impl Into<String>) -> Self {
    Self {
      token: token.into(),
      console: None,
    }
  }

  /// The bearers with the console relay's credential beside the receiver's token; an empty one, or one that is the
  /// receiver's own token, names none.
  pub fn console(mut self, console: Option<impl Into<String>>) -> Self {
    self.console = console.map(Into::into).filter(|token| !token.is_empty() && *token != self.token);

    self
  }
}

impl From<&str> for Bearers {
  fn from(token: &str) -> Self {
    Self::new(token)
  }
}

/// Which credential a request was let in under, which the bearer check leaves on the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Credential {
  /// The receiver's own token, which every unit and every shell of the machine holds.
  Receiver,
  /// The console relay's own credential.
  Console,
}

/// A refusal: its status beside its code and the sentence naming it.
struct Refusal {
  status: StatusCode,
  code: String,
  error: String,
}

fn refused(status: StatusCode, code: &str, error: &str) -> Refusal {
  Refusal {
    status,
    code: code.to_owned(),
    error: error.to_owned(),
  }
}

/// Whether the web unit says the named console user is the platform administrator.
pub(super) async fn is_administrator(receiver: &Receiver, by: &str) -> bool {
  matches!(receiver.units().web.user(by).await, Ok(found) if found.administrator)
}

impl IntoResponse for Refusal {
  fn into_response(self) -> Response {
    let body = Json(json!({ "code": self.code, "error": self.error }));

    (self.status, body).into_response()
  }
}

/// An answer with a body, or the refusal.
type Answered = Result<Json<serde_json::Value>, Refusal>;

/// An answer with its status beside the body, for the routes that answer 201 or 204.
type Accepted = Result<(StatusCode, Json<serde_json::Value>), Refusal>;

fn answered(body: serde_json::Value) -> Answered {
  Ok(Json(body))
}

/// A route that takes files, reading its form under no ceiling of the receiver's own rather than the framework's
/// default: the largest file a room takes is its chat providers' to declare (docs/api.md, 「Sends」).
fn taking_files(router: MethodRouter<Served>) -> MethodRouter<Served> {
  router.layer(DefaultBodyLimit::disable())
}

/// A route that files a person's words in a room or to a room's session: answered under the console relay's
/// credential alone, and refused under the receiver's own token before anything is read (docs/api.md, 「Auth」).
fn persons_words(router: MethodRouter<Served>) -> MethodRouter<Served> {
  router.layer(from_fn(console_only))
}

async fn console_only(request: Request, next: Next) -> Response {
  match request.extensions().get::<Credential>() {
    Some(Credential::Console) => next.run(request).await,
    _ => refused(
      StatusCode::FORBIDDEN,
      "forbidden",
      "A person's words are taken under the console relay's credential alone, never under the receiver's token.",
    )
    .into_response(),
  }
}

/// Every route the receiver answers behind its token: its method, its path and the handler standing at it.
/// The table is what the `routes` command prints, so the router is built from it rather than beside it.
fn table() -> Vec<(String, String, MethodRouter<Served>)> {
  [
    ("GET", "/health", get(events::health)),
    ("GET", "/owner", get(rooms::owner)),
    ("GET", "/rooms", get(console::rooms)),
    ("POST", "/rooms", post(rooms::add_room)),
    ("GET", "/rooms/{roomId}", get(rooms::room)),
    ("GET", "/rooms/{roomId}/registration", get(rooms::room_registration)),
    ("PUT", "/rooms/{roomId}/registration", put(rooms::change_room_mode)),
    ("GET", "/rooms/{roomId}/archive", get(rooms::room_archive)),
    ("GET", "/rooms/{roomId}/leaders", get(rooms::room_leaders)),
    ("PUT", "/rooms/{roomId}/archive", put(rooms::change_room_archive)),
    ("GET", "/rooms/{roomId}/title", get(console::title)),
    ("PUT", "/rooms/{roomId}/title", put(rooms::change_room_title)),
    ("GET", "/rooms/{roomId}/tone", get(tones::room_tone)),
    ("PUT", "/rooms/{roomId}/tone", put(tones::change_room_tone)),
    ("GET", "/answerers", get(brain::all_answerers)),
    ("GET", "/rooms/{roomId}/answerers", get(rooms::answerers)),
    ("GET", "/rooms/{roomId}/messages", get(console::messages)),
    ("GET", "/rooms/{roomId}/messages/{messageId}", get(rooms::message)),
    ("GET", "/rooms/{roomId}/messages/{messageId}/metadata", get(console::metadata)),
    (
      "GET",
      "/rooms/{roomId}/messages/{messageId}/sender-is-owner",
      get(brain::sender_is_owner),
    ),
    ("GET", "/rooms/{roomId}/messages/{messageId}/files", get(message_files::files)),
    ("GET", "/rooms/{roomId}/messages/{messageId}/files/{place}", get(message_files::file)),
    ("GET", "/rooms/{roomId}/messages/{messageId}/picture", get(message_files::picture)),
    ("POST", "/rooms/{roomId}/file-fills", post(file_fills::fill)),
    ("GET", "/rooms/{roomId}/day-start", get(console::day_start)),
    ("GET", "/rooms/{roomId}/pictures", get(console::pictures)),
    ("GET", "/avatars", get(message_files::avatars)),
    ("GET", "/rooms/{roomId}/avatar", get(message_files::room_avatar)),
    ("GET", "/people/{personId}/avatar", get(message_files::person_avatar)),
    ("PUT", "/avatars/{kind}/{subject}", taking_files(put(avatars::set))),
    (
      "POST",
      "/rooms/{roomId}/messages/{messageId}/forwards",
      persons_words(post(say::forward)),
    ),
    ("GET", "/rooms/{roomId}/pins", get(message_acts::pins)),
    ("PUT", "/rooms/{roomId}/messages/{messageId}/reactions/mine", put(message_acts::react)),
    (
      "DELETE",
      "/rooms/{roomId}/messages/{messageId}/reactions/mine",
      delete(message_acts::unreact),
    ),
    ("PUT", "/rooms/{roomId}/messages/{messageId}/pin", put(message_acts::pin)),
    (
      "PATCH",
      "/rooms/{roomId}/messages/{messageId}",
      persons_words(patch(message_acts::edit)),
    ),
    ("PUT", "/rooms/{roomId}/messages/{messageId}/speaker", put(speakers::change_speaker)),
    ("DELETE", "/rooms/{roomId}/messages/{messageId}", delete(message_acts::delete)),
    ("GET", "/stickers", get(message_acts::stickers)),
    ("POST", "/rooms/{roomId}/stickers", post(message_acts::sticker)),
    ("DELETE", "/avatars/{kind}/{subject}", delete(avatars::unset)),
    ("GET", "/rooms/{roomId}/figures", get(figures::figures)),
    ("GET", "/rooms/{roomId}/window", get(console::room_window)),
    ("GET", "/rooms/{roomId}/situation", get(console::room_situation)),
    ("GET", "/providers", get(rooms::providers)),
    ("GET", "/providers/{key}/rooms", get(rooms::account_rooms)),
    ("GET", "/providers/{key}/contacts", get(rooms::contacts)),
    ("GET", "/rooms/{roomId}/providers", get(rooms::room_providers)),
    ("PUT", "/rooms/{roomId}/providers", put(rooms::choose_providers)),
    ("GET", "/rooms/{roomId}/members", get(rooms::room_members)),
    ("GET", "/rooms/{roomId}/accounts", get(accounts::members)),
    ("PUT", "/rooms/{roomId}/accounts/{account}", put(accounts::let_in)),
    ("POST", "/rooms/{roomId}/accounts", post(accounts::let_in_several)),
    ("POST", "/rooms/{roomId}/unit-accounts", post(accounts::let_in_unit)),
    ("DELETE", "/rooms/{roomId}/accounts/{account}", delete(accounts::take_out)),
    ("PUT", "/accounts/{account}/identity", put(accounts::identity)),
    ("GET", "/accounts/{account}/rooms", get(accounts::rooms)),
    ("PUT", "/accounts/{account}/presence", put(presence::account_presence)),
    ("POST", "/rooms/{roomId}/typing", post(presence::account_typing)),
    ("GET", "/rooms/{roomId}/roads", get(brain::roads)),
    ("GET", "/rooms/{roomId}/own-number", get(brain::own_number)),
    ("GET", "/rooms/{roomId}/spoke-between", get(brain::spoke_between)),
    ("GET", "/rooms/{roomId}/people", get(brain::spoken_in)),
    ("GET", "/rooms/{roomId}/owner-spoke", get(brain::owner_spoke)),
    ("GET", "/rooms/{roomId}/read-marks", get(brain::read_count)),
    ("POST", "/rooms/{roomId}/read-marks", post(brain::keep)),
    ("DELETE", "/rooms/{roomId}/read-marks", delete(brain::forget)),
    ("POST", "/read-mark-counts", post(brain::unread_counts)),
    ("GET", "/rooms/{roomId}/notices", get(instructions::notices)),
    ("POST", "/rooms/{roomId}/notices", post(instructions::notice)),
    ("GET", "/rooms/{roomId}/send-verbs", get(sends::can)),
    ("GET", "/people", get(brain::names)),
    ("POST", "/promise-readings", post(brain::waits)),
    ("POST", "/instructions", post(instructions::queue_instruction)),
    ("GET", "/instructions/{instructionId}", get(instructions::instruction_state)),
    ("DELETE", "/instructions/{instructionId}", delete(instructions::withdraw)),
    ("POST", "/owner/notices", post(brain::notice)),
    ("POST", "/owner/instructions", persons_words(post(instruct))),
    ("PUT", "/room-holders/{holderName}", put(instructions::declare)),
    ("POST", "/sends", post(sends)),
    ("POST", "/owner/messages", persons_words(post(say))),
    ("POST", "/owner/message-files", persons_words(taking_files(post(say_file)))),
    ("POST", "/rooms/{roomId}/words", persons_words(post(accounts::words))),
    (
      "POST",
      "/rooms/{roomId}/word-files",
      persons_words(taking_files(post(accounts::word_files))),
    ),
    ("POST", "/talks", post(talk)),
    ("POST", "/handoffs/{agentName}", post(handoffs::hand_off)),
    ("POST", "/handovers/{agentName}", post(handovers::hand_over)),
    ("GET", "/voice-briefing", get(briefing::switches)),
    ("PUT", "/voice-briefing", put(briefing::switch)),
    ("POST", "/rooms/{roomId}/messages/{messageId}/reading", post(briefing::reading)),
    ("POST", "/events", post(events::told)),
    ("GET", "/events", get(events::stream)),
    ("GET", "/live", get(events::live)),
    ("POST", "/hooks/{point}", post(hooks::hook)),
    ("DELETE", "/rooms/{roomId}", delete(room_removal::remove)),
    ("GET", "/rounds/openings", get(openings::openings)),
    ("GET", "/rounds/openings/{instructionId}", get(openings::instruction)),
    ("GET", "/rounds/pipeline", get(stages::rounds)),
    ("GET", "/instruction-stages", get(stages::chains)),
    ("PUT", "/instruction-stages/{agentName}", put(stages::set_chain)),
    ("DELETE", "/instruction-stages/{agentName}", delete(stages::remove_chain)),
    ("GET", "/instruction-texts", get(stages::texts)),
    ("PUT", "/instruction-texts/{agentName}/{name}", put(stages::set_text)),
    ("DELETE", "/instruction-texts/{agentName}/{name}", delete(stages::remove_text)),
    ("POST", "/supervisors", post(supervisors::add)),
    ("DELETE", "/supervisors/{supervisorName}", delete(supervisors::remove)),
    ("POST", "/supervisors/{supervisorName}/resets", post(supervisors::reset)),
    ("GET", "/supervisors/{supervisorName}/rooms", get(supervisors::rooms)),
    (
      "GET",
      "/supervisors/{supervisorName}/rooms/{roomId}/messages",
      get(supervisors::member_messages),
    ),
    ("GET", "/default-answerer", get(supervisors::default_answerer)),
    ("PUT", "/default-answerer", put(supervisors::set_default_answerer)),
    ("PUT", "/rooms/{roomId}/supervisors", put(supervisors::name_room)),
    ("PUT", "/pinned-rooms", put(console::pin)),
    ("POST", "/pinned-rooms", post(console::pin_one)),
    ("DELETE", "/pinned-rooms/{room}", delete(console::unpin)),
  ]
  .into_iter()
  .map(|(method, path, router)| (method.to_owned(), path.to_owned(), router))
  .collect()
}

/// Every route the receiver answers, one method and path to a name: its own table, the chat services' hand-overs
/// served by the base library, and the warmer's hook, which stands outside the bearer check.
pub fn served() -> Vec<String> {
  let names = table()
    .iter()
    .map(|(method, path, _)| format!("{method} {path}"))
    .collect::<Vec<String>>();

  let mut printed = names
    .into_iter()
    .chain(
      [
        "GET /chat/rooms",
        "GET /chat/rooms/{room_id}",
        "PUT /chat/rooms/{room_id}/messages/{message_id}",
        "PATCH /chat/rooms/{room_id}/messages/{message_id}",
        "POST /chat/avatar-changes",
        "POST /chat/reaction-changes",
        "POST /chat/deletions",
        "POST /chat/typing-marks",
        "POST /chat/presence-changes",
        "POST /warmer/ping",
      ]
      .into_iter()
      .map(|name| name.to_owned()),
    )
    .collect::<Vec<String>>();

  printed.sort();

  printed
}

/// The receiver's routes, merged.
pub fn routes(receiver: Receiver, bearers: impl Into<Bearers>) -> Router {
  let bearers: Bearers = bearers.into();
  let served = Served {
    receiver: Arc::new(receiver.clone()),
    token: bearers.token.into(),
    console: bearers.console.map(Into::into),
  };

  let served_routes = table()
    .into_iter()
    .fold(Router::new(), |app, (_, path, router)| app.route(path.as_str(), router))
    .fallback(no_route)
    .method_not_allowed_fallback(no_route)
    .layer(from_fn_with_state(served.clone(), authorized))
    .with_state(served.clone());

  // The warmer's hook: it sends no header at all, so the token stands in the URL the receiver registered and
  // this route alone reads it, outside the bearer check every other route answers behind.
  let hook = Router::new().route("/warmer/ping", post(hooks::ping)).with_state(served);

  served_routes.merge(hook)
}

/// A request that carries neither the receiver's token nor the console relay's is answered with `unauthorized`;
/// one that does is let in with the credential it carried left on it.
async fn authorized(State(served): State<Served>, mut request: Request, next: Next) -> Response {
  let bearer = request.headers().typed_get::<Authorization<Bearer>>();
  let carried = bearer.as_ref().map(|Authorization(bearer)| bearer.token());
  let credential = match carried {
    Some(token) if served.console.as_deref() == Some(token) => Some(Credential::Console),
    Some(token) if token == &*served.token => Some(Credential::Receiver),
    _ => None,
  };

  match credential {
    Some(credential) => {
      request.extensions_mut().insert(credential);
      next.run(request).await
    }
    None => refused(StatusCode::UNAUTHORIZED, "unauthorized", "The bearer token is missing or wrong.")
      .into_response(),
  }
}

async fn no_route(request: Request) -> Answered {
  let where_none = format!("There is no route at {} {}.", request.method(), request.uri().path());

  Err(refused(StatusCode::NOT_FOUND, "no-route", &where_none))
}

/// The query read as the shape the route takes; a query naming nothing reads as the shape's defaults.
fn query<T: serde::de::DeserializeOwned>(raw: RawQuery) -> Result<T, Refusal> {
  let text = raw.0.unwrap_or_default();

  serde_urlencoded::from_str(&text).map_err(|error| {
    let why = format!("The query is not the shape this route takes: {error}.");

    refused(StatusCode::BAD_REQUEST, "bad-request", &why)
  })
}

/// The body read as the shape the route takes.
fn read<T: serde::de::DeserializeOwned>(body: &Bytes) -> Result<T, Refusal> {
  serde_json::from_slice(body).map_err(|error| {
    let why = format!("The body is not the shape this route takes: {error}.");

    refused(StatusCode::BAD_REQUEST, "bad-request", &why)
  })
}

/// A refusal the receiver's own codes give, at its status and under its code.
fn code<T: Refused>(why: &T) -> Refusal {
  let sentence = why.to_string();

  refused(why.status(), why.code(), &sentence)
}

/// A call on another unit that did not answer.
fn unit(why: PortFailure) -> Refusal {
  let sentence = why.to_string();

  refused(StatusCode::BAD_GATEWAY, "upstream-failed", &sentence)
}

/// The store failed while answering.
fn store(why: Fault) -> Refusal {
  refused(StatusCode::INTERNAL_SERVER_ERROR, "failed", &why.detail)
}

/// The receiver is up but a part it needs does not stand yet.
fn ready(what: &str) -> Refusal {
  let why = format!("{} are not standing.", what);

  refused(StatusCode::SERVICE_UNAVAILABLE, "not-ready", &why)
}

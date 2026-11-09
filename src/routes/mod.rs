//! The receiver's own routes (docs/boundary-api.md, 「The receiver's own routes」): what the console, the send
//! tool and the `bmo` command line reach the receiver on. Every one answers only a request carrying the
//! receiver's token, and a refusal with its code and the English sentence naming it.

mod as_owner;
mod avatars;
mod brain;
mod briefing;
mod console;
mod drafts;
mod events;
mod figures;
mod handoffs;
mod handovers;
mod hooks;
mod instructions;
mod message_acts;
mod message_files;
mod openings;
mod owner_words;
mod room_removal;
mod rooms;
mod say;
mod sends;
mod supervisors;
mod talks;

use std::sync::Arc;

use axum::{
  Router,
  body::Bytes,
  extract::{RawQuery, Request, State},
  http::StatusCode,
  middleware::{Next, from_fn_with_state},
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

/// What every route stands on: the receiver and the token it answers behind.
#[derive(Clone)]
struct Served {
  receiver: Arc<Receiver>,
  token: Arc<str>,
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
    ("GET", "/rooms/{roomId}/title", get(console::title)),
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
    ("GET", "/rooms/{roomId}/day-start", get(console::day_start)),
    ("GET", "/rooms/{roomId}/pictures", get(console::pictures)),
    ("GET", "/avatars", get(message_files::avatars)),
    ("GET", "/rooms/{roomId}/avatar", get(message_files::room_avatar)),
    ("GET", "/people/{personId}/avatar", get(message_files::person_avatar)),
    ("PUT", "/avatars/{kind}/{subject}", put(avatars::set)),
    ("POST", "/rooms/{roomId}/messages/{messageId}/forwards", post(say::forward)),
    ("GET", "/rooms/{roomId}/pins", get(message_acts::pins)),
    ("PUT", "/rooms/{roomId}/messages/{messageId}/reactions/mine", put(message_acts::react)),
    (
      "DELETE",
      "/rooms/{roomId}/messages/{messageId}/reactions/mine",
      delete(message_acts::unreact),
    ),
    ("PUT", "/rooms/{roomId}/messages/{messageId}/pin", put(message_acts::pin)),
    ("PATCH", "/rooms/{roomId}/messages/{messageId}", patch(message_acts::edit)),
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
    ("POST", "/owner/instructions", post(instruct)),
    ("PUT", "/room-holders/{holderName}", put(instructions::declare)),
    ("POST", "/sends", post(sends)),
    ("POST", "/owner/messages", post(say)),
    ("POST", "/owner/message-files", post(say_file)),
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
    ("GET", "/rounds/drafts", get(drafts::drafts)),
    ("GET", "/draft-processes", get(drafts::processes)),
    ("PUT", "/draft-processes/{provider}/{tier}/{agentName}", put(drafts::set_process)),
    (
      "DELETE",
      "/draft-processes/{provider}/{tier}/{agentName}",
      delete(drafts::remove_process),
    ),
    ("GET", "/draft-questions", get(drafts::questions)),
    ("PUT", "/draft-questions/{name}", put(drafts::set_question)),
    ("DELETE", "/draft-questions/{name}", delete(drafts::remove_question)),
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
pub fn routes(receiver: Receiver, token: impl Into<String>) -> Router {
  let bearer: String = token.into();
  let served = Served {
    receiver: Arc::new(receiver.clone()),
    token: bearer.clone().into(),
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

/// A request that does not carry the receiver's token is answered with `unauthorized`.
async fn authorized(State(served): State<Served>, request: Request, next: Next) -> Response {
  let bearer = request.headers().typed_get::<Authorization<Bearer>>();

  match bearer {
    Some(Authorization(bearer)) if bearer.token() == &*served.token => next.run(request).await,
    _ => refused(StatusCode::UNAUTHORIZED, "unauthorized", "The bearer token is missing or wrong.")
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

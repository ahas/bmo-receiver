//! The receiver's own routes (docs/boundary-api.md, 「The receiver's own routes」): what the console, the send
//! tool and the `bmo` command line reach the receiver on. Every one answers only a request carrying the
//! receiver's token, and a refusal with its code and the English sentence naming it.

mod as_owner;
mod brain;
mod companions;
mod console;
mod consult;
mod events;
mod hooks;
mod instructions;
mod message_files;
mod openings;
mod owner_words;
mod room_removal;
mod rooms;
mod say;
mod sends;
mod talks;
mod voice;

use std::sync::Arc;

use axum::{
  Router,
  body::Bytes,
  extract::{Request, State},
  http::StatusCode,
  middleware::{Next, from_fn_with_state},
  response::{IntoResponse, Json, Response},
  routing::MethodRouter,
  routing::{delete, get, post, put},
};
use headers::authorization::Bearer;
use headers::{Authorization, HeaderMapExt};
use serde_json::json;

use receiver_codes::Refused;
use receiver_ports::http::PortFailure;

use crate::Fault;
use crate::receiver::Receiver;
use crate::routes::consult::{block, consult_say};
use crate::routes::owner_words::instruct;
use crate::routes::say::{say, say_file};
use crate::routes::sends::{can, sends};
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

impl IntoResponse for Refusal {
  fn into_response(self) -> Response {
    let body = Json(json!({ "code": self.code, "error": self.error }));

    (self.status, body).into_response()
  }
}

/// An answer with a body, or the refusal.
type Answered = Result<Json<serde_json::Value>, Refusal>;

fn answered(body: serde_json::Value) -> Answered {
  Ok(Json(body))
}

/// Every route the receiver answers behind its token: its method, its path and the handler standing at it.
/// The table is what the `routes` command prints, so the router is built from it rather than beside it.
fn table() -> Vec<(String, String, MethodRouter<Served>)> {
  [
    ("POST", "/consult/say", post(consult_say)),
    ("POST", "/consult/block", post(block)),
    ("POST", "/instruct", post(instruct)),
    ("POST", "/say", post(say)),
    ("POST", "/say-file", post(say_file)),
    ("POST", "/sends", post(sends)),
    ("POST", "/sends/can", post(can)),
    ("POST", "/talks", post(talk)),
    ("POST", "/instructions", post(instructions::queue_instruction)),
    ("GET", "/instructions/{id}", get(instructions::instruction_state)),
    ("DELETE", "/instructions/{id}", delete(instructions::withdraw)),
    ("POST", "/rooms/{chat}/notices", post(instructions::notice)),
    ("PUT", "/room-holders/{unit}", put(instructions::declare)),
    ("GET", "/owner", get(rooms::owner)),
    ("GET", "/rooms/{chat}", get(rooms::room)),
    ("GET", "/rooms/{chat}/answerers", get(rooms::answerers)),
    ("GET", "/rooms/{chat}/messages/{message}", get(rooms::message)),
    ("POST", "/messages/unread", post(brain::unread)),
    ("POST", "/messages/unread-counts", post(brain::unread_counts)),
    ("POST", "/messages/read", post(brain::read_messages)),
    ("POST", "/read-marks/count", post(brain::read_count)),
    ("POST", "/read-marks/keep", post(brain::keep)),
    ("POST", "/read-marks/forget", post(brain::forget)),
    ("POST", "/rooms/titles", post(brain::titles)),
    ("POST", "/rooms/answerers", post(brain::answerers)),
    ("POST", "/rooms/spoke-between", post(brain::spoke_between)),
    ("POST", "/rooms/own-number", post(brain::own_number)),
    ("POST", "/rooms/spoken-in", post(brain::spoken_in)),
    ("POST", "/people/names", post(brain::names)),
    ("POST", "/promises/waits", post(brain::waits)),
    ("POST", "/rooms/roads", post(brain::roads)),
    ("POST", "/messages/sender-is-owner", post(brain::sender_is_owner)),
    ("POST", "/rooms/owner-spoke", post(brain::owner_spoke)),
    ("POST", "/owner/notice", post(brain::notice)),
    ("POST", "/instructions/quiet", post(brain::quiet)),
    ("POST", "/agency/hooks/{point}", post(hooks::hook)),
    ("POST", "/event", post(events::told)),
    ("GET", "/events", get(events::stream)),
    ("GET", "/live", get(events::live)),
    ("GET", "/health", get(events::health)),
    ("GET", "/rooms/remove", get(room_removal::preview)),
    ("POST", "/rooms/remove", post(room_removal::remove)),
    ("GET", "/rounds/openings", get(openings::openings)),
    ("GET", "/rounds/instruction", get(openings::instruction)),
    ("GET", "/voice-briefing", get(voice::state)),
    ("POST", "/voice-briefing", post(voice::switch)),
    ("POST", "/voice-briefing/line", post(voice::line)),
    ("POST", "/companions/add", post(companions::add)),
    ("POST", "/companions/remove", post(companions::remove)),
    ("GET", "/console/rooms", get(console::rooms)),
    ("GET", "/console/rooms/{chat}/title", get(console::title)),
    ("GET", "/console/rooms/{chat}/agent-message/{message}", get(console::agent_message)),
    ("GET", "/console/rooms/{chat}/messages", get(console::page)),
    ("GET", "/console/rooms/{chat}/day-start", get(console::day_start)),
    ("GET", "/console/rooms/{chat}/messages/after", get(console::after)),
    ("GET", "/console/rooms/{chat}/messages/{message}", get(console::one)),
    ("GET", "/console/rooms/{chat}/messages/{message}/files", get(message_files::files)),
    (
      "GET",
      "/console/rooms/{chat}/messages/{message}/files/{place}",
      get(message_files::file),
    ),
    ("GET", "/console/rooms/{chat}/messages/{message}/picture", get(message_files::picture)),
    ("GET", "/console/rooms/{chat}/pictures", get(console::pictures)),
    ("GET", "/console/rooms/{chat}/window", get(console::room_window)),
    ("PUT", "/console/pinned-rooms", put(console::pin)),
    ("POST", "/console/pinned-rooms/add", post(console::pin_one)),
    ("POST", "/console/pinned-rooms/remove", post(console::unpin)),
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
        "POST /message",
        "POST /attachment",
        "POST /edited",
        "POST /gathered",
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

  refused(StatusCode::BAD_GATEWAY, "unit-failed", &sentence)
}

/// The store failed while answering.
fn store(why: Fault) -> Refusal {
  refused(StatusCode::INTERNAL_SERVER_ERROR, "failed", &why.detail)
}

/// The receiver is up but a part it needs does not stand yet.
fn ready(what: &str) -> Refusal {
  let why = format!("{} are not standing.", what);

  refused(StatusCode::CONFLICT, "not-ready", &why)
}

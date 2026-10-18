//! The receiver's own routes (docs/boundary-api.md, 「The receiver's own routes」): what the console, the send
//! tool and the `bmo` command line reach the receiver on. Every one answers only a request carrying the
//! receiver's token, and a refusal with its code and the English sentence naming it.

mod as_owner;
mod brain;
mod console;
mod consult;
mod hooks;
mod instructions;
mod owner_words;
mod rooms;
mod say;
mod sends;
mod talks;

use std::sync::Arc;

use axum::{
  Router,
  body::Bytes,
  extract::{Request, State},
  http::StatusCode,
  middleware::{Next, from_fn_with_state},
  response::{IntoResponse, Json, Response},
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

/// The receiver's routes, merged.
pub fn routes(receiver: Receiver, token: impl Into<String>) -> Router {
  let bearer: String = token.into();
  let served = Served {
    receiver: Arc::new(receiver.clone()),
    token: bearer.clone().into(),
  };

  let served_routes = Router::new()
    .route("/consult/say", post(consult_say))
    .route("/consult/block", post(block))
    .route("/instruct", post(instruct))
    .route("/say", post(say))
    .route("/say-file", post(say_file))
    .route("/sends", post(sends))
    .route("/sends/can", post(can))
    .route("/talks", post(talk))
    .route("/instructions", post(instructions::queue_instruction))
    .route("/instructions/{id}", get(instructions::instruction_state))
    .route("/instructions/{id}", delete(instructions::withdraw))
    .route("/rooms/{chat}/notices", post(instructions::notice))
    .route("/room-holders/{unit}", put(instructions::declare))
    .route("/owner", get(rooms::owner))
    .route("/rooms/{chat}", get(rooms::room))
    .route("/rooms/{chat}/answerers", get(rooms::answerers))
    .route("/rooms/{chat}/messages/{message}", get(rooms::message))
    .route("/messages/unread", post(brain::unread))
    .route("/messages/unread-counts", post(brain::unread_counts))
    .route("/messages/read", post(brain::read_messages))
    .route("/read-marks/count", post(brain::read_count))
    .route("/read-marks/keep", post(brain::keep))
    .route("/read-marks/forget", post(brain::forget))
    .route("/rooms/titles", post(brain::titles))
    .route("/rooms/answerers", post(brain::answerers))
    .route("/rooms/spoke-between", post(brain::spoke_between))
    .route("/rooms/own-number", post(brain::own_number))
    .route("/rooms/spoken-in", post(brain::spoken_in))
    .route("/people/names", post(brain::names))
    .route("/promises/waits", post(brain::waits))
    .route("/rooms/roads", post(brain::roads))
    .route("/messages/sender-is-owner", post(brain::sender_is_owner))
    .route("/rooms/owner-spoke", post(brain::owner_spoke))
    .route("/owner/notice", post(brain::notice))
    .route("/instructions/quiet", post(brain::quiet))
    .route("/agency/hooks/{point}", post(hooks::hook))
    .route("/console/rooms", get(console::rooms))
    .route("/console/rooms/{chat}/title", get(console::title))
    .route("/console/rooms/{chat}/agent-message/{message}", get(console::agent_message))
    .route("/console/rooms/{chat}/messages", get(console::page))
    .route("/console/rooms/{chat}/day-start", get(console::day_start))
    .route("/console/rooms/{chat}/messages/after", get(console::after))
    .route("/console/rooms/{chat}/messages/{message}", get(console::one))
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

//! One screen's stream as it goes out: the opening, then each frame as the stream is told it, with a
//! ping between. A stream sends nothing between pings, so the idle limit of whatever carries it has to
//! outlast one.

use std::convert::Infallible;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::response::{IntoResponse, Response};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{Instant, Interval, MissedTickBehavior, interval_at};

use super::STOPPING_FRAME;

const PING: Duration = Duration::from_secs(15);
const PING_FRAME: &[u8] = b": ping\n\n";

/// The stream answered: it ends once the stream lets its listener go, after the frames it was told.
pub(super) fn response(opening: String, frames: UnboundedReceiver<String>) -> Response {
  let mut ping = interval_at(Instant::now() + PING, PING);

  ping.set_missed_tick_behavior(MissedTickBehavior::Delay);

  let body = futures_util::stream::unfold(
    Going {
      opening: Some(opening),
      frames,
      ping,
      ended: false,
    },
    next,
  );

  (
    [("content-type", "text/event-stream"), ("cache-control", "no-cache")],
    Body::from_stream(body),
  )
    .into_response()
}

struct Going {
  opening: Option<String>,
  frames: UnboundedReceiver<String>,
  ping: Interval,
  // The word that the process is stopping went out, which is the stream's last.
  ended: bool,
}

async fn next(mut going: Going) -> Option<(Result<Bytes, Infallible>, Going)> {
  if going.ended {
    return None;
  }

  if let Some(opening) = going.opening.take() {
    return Some((Ok(Bytes::from(opening)), going));
  }

  let chunk = tokio::select! {
    frame = going.frames.recv() => {
      let frame = frame?;

      going.ended = frame == STOPPING_FRAME;
      Bytes::from(frame)
    }
    _ = going.ping.tick() => Bytes::from_static(PING_FRAME),
  };

  Some((Ok(chunk), going))
}

//! A turn followed to its end across cuts (docs/bmo-window.md, 「Quiet instruction」): a turn is a resource the
//! round re-attaches to, never one held call. The round follows the turn's events on the agency's stream; a stream
//! that ends or fails, or a send whose connection closed, is no end of the turn. A look that carried no words to
//! the named turn reads it at the agency (`Followed::probe`), so an end the stream loses cannot pass unheard. The
//! round then opens the stream again and reads the events the agency kept since the last one heard, where the
//! turn's end stands once it came; where none does it reads the turn: one still running is followed on, one whose
//! read carries its end gives it, and one read as ended with no end anywhere, or one the agency holds no record
//! of, fails the round, uncounted. While the agency answers nothing the round keeps asking, and fails, uncounted,
//! only past the policy's while.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use futures_util::StreamExt;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_ports::agency::{Agency, Event, Events};
use receiver_ports::http::PortFailure;
use serde_json::{Value, json};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::conversation::Failure;

/// Whether the agency's answer is no answer at all: nothing listened, the connection was cut, or what stands in
/// front of the agency said it is not there yet. Such a failure is never a final outcome of a turn.
pub fn unreachable(why: &PortFailure) -> bool {
  match why {
    PortFailure::Unreachable { .. } | PortFailure::Transport { .. } => true,
    PortFailure::Answered { status, .. } => (502..=504).contains(status),
    PortFailure::NotAnAddress { .. } | PortFailure::Undecodable { .. } => false,
  }
}

/// The failure a call to the agency ended a turn with: uncounted where the agency did not answer, counted where it
/// did.
pub fn failure_of(why: PortFailure) -> Failure {
  if unreachable(&why) {
    Failure::Transport(why.to_string())
  } else {
    Failure::Failed(why.to_string())
  }
}

/// What following the turn came to: one of its session's events, its end, or word that no event the agency kept
/// names a turn of the session since the turn went out, so a send that was cut may go again under its key.
#[derive(Debug)]
pub enum Step {
  Event(Event),
  Ended(Value),
  Unnamed,
}

/// What reading the named turn at the agency came to: the turn still going, the end the read itself carried,
/// or the turn gone — read as ended with nothing carrying its end, or answered with no record of it.
#[derive(Debug)]
pub enum Probed {
  Going,
  Ended(Value),
  Gone,
}

/// The turn's id and run, once the agency named them, and the number of the last event heard.
#[derive(Clone, Debug, Default)]
struct Named {
  turn: Option<i64>,
  run: Option<String>,
  mark: Option<i64>,
}

/// The turn a round follows on one session, followed on a task of its own so that no step of a re-attachment is
/// ever dropped halfway: the round reads its steps as they come, tells it of a cut send, and reads the turn's id
/// and run once the agency named them. The following stops once the turn ended or failed, or the round lets it go.
pub struct Followed {
  agency: Arc<dyn Agency>,
  session: String,
  steps: mpsc::UnboundedReceiver<Result<Step, Failure>>,
  cuts: Arc<Notify>,
  named: Arc<Mutex<Named>>,
  following: JoinHandle<()>,
}

impl Followed {
  /// The turn of `session` followed from the event numbered `mark`, the last heard before it went out, on a
  /// stream opened now; a stream that does not open is opened again as the first step asks for it, `pace` apart
  /// while the agency answers nothing.
  pub async fn open(
    agency: Arc<dyn Agency>,
    session: &str,
    kinds: &'static [&'static str],
    mark: Option<i64>,
    pace: Duration,
  ) -> Self {
    let stream = match agency.events(kinds).await {
      Ok(stream) => Some(stream),
      Err(why) => {
        eprintln!("session {session}: the agency's events were not opened for its turn: {why}");
        None
      }
    };
    let named = Arc::new(Mutex::new(Named::default()));
    let cuts = Arc::new(Notify::new());
    let (told, steps) = mpsc::unbounded_channel();
    let attached = Attached {
      agency: agency.clone(),
      session: session.to_owned(),
      kinds,
      stream,
      mark,
      cut_since: None,
      pace,
      bound: Duration::from_millis(CHAT_ROUND.attach.reattach_ms),
      named: named.clone(),
    };
    let following = tokio::spawn(attached.follow(told, cuts.clone()));

    Followed {
      agency,
      session: session.to_owned(),
      steps,
      cuts,
      named,
      following,
    }
  }

  /// What a look at the named turn standing gone comes to, read where the stream has said nothing: the turn's
  /// end where the read itself carries it, nothing where the turn still runs or the agency answered nothing,
  /// and gone where the read says ended with no end the read carries or the agency holds no record of it — a
  /// turn gone is cut, and the re-attachment reads what became of it off the events kept or the turn, failing
  /// the round uncounted where nothing carries its end any more.
  pub async fn probe(&self) -> Probed {
    let Some(turn) = self.turn() else { return Probed::Going };

    match self.agency.turn_read(&self.session, turn).await {
      // A read carrying the end gives it whole, as the stream's own end would.
      Ok(read) if read.ended() && read.outcome.is_some() => Probed::Ended(json!({
        "session": self.session,
        "turnId": turn,
        "outcome": read.outcome,
        "answer": read.answer,
        "reason": read.reason,
        "delivery": read.delivery,
      })),
      Ok(read) if read.ended() => Probed::Gone,
      Ok(_) => Probed::Going,
      Err(why) if unreachable(&why) => Probed::Going,
      Err(why) => {
        eprintln!("session {}: the turn read answered nothing of it: {why}", self.session);

        Probed::Gone
      }
    }
  }

  /// The next step of the turn: an event of its session, or its end, re-attaching across every cut; a failure,
  /// uncounted, only where the agency holds no record of the turn or did not answer for the policy's while. Its
  /// future may be dropped at any point and asked again, losing nothing.
  pub async fn next(&mut self) -> Result<Step, Failure> {
    self
      .steps
      .recv()
      .await
      .unwrap_or_else(|| Err(Failure::Transport("the turn's following ended with no end of it".to_owned())))
  }

  /// The send's connection was cut: the stream is let go and the next step re-attaches, so a turn the agency never
  /// named is found unnamed and sent again, whichever of the stream and the send the cut reached first.
  pub fn cut(&self) {
    self.cuts.notify_one();
  }

  /// The turn's id, as `turn.created` named it.
  pub fn turn(&self) -> Option<i64> {
    self.named.lock().unwrap_or_else(PoisonError::into_inner).turn
  }

  /// The turn's run, as `turn.created` named it.
  pub fn run(&self) -> Option<String> {
    self.named.lock().unwrap_or_else(PoisonError::into_inner).run.clone()
  }

  /// The number of the last of the agency's events the following heard, where one carried a number.
  pub fn mark(&self) -> Option<i64> {
    self.named.lock().unwrap_or_else(PoisonError::into_inner).mark
  }
}

impl Drop for Followed {
  fn drop(&mut self) {
    self.following.abort();
  }
}

// The following itself: the events of the kinds it reads, the last event heard, and how long the agency has not
// answered since a cut.
struct Attached {
  agency: Arc<dyn Agency>,
  session: String,
  kinds: &'static [&'static str],
  stream: Option<Events>,
  mark: Option<i64>,
  cut_since: Option<Instant>,
  pace: Duration,
  bound: Duration,
  named: Arc<Mutex<Named>>,
}

impl Attached {
  // Every step told to the round as it comes, a cut send re-attaching, until the turn ended or failed or the round
  // no longer listens.
  async fn follow(mut self, told: mpsc::UnboundedSender<Result<Step, Failure>>, cuts: Arc<Notify>) {
    loop {
      let step = tokio::select! {
        step = self.next() => step,
        () = cuts.notified() => {
          self.cut();
          continue;
        }
      };
      let last = !matches!(step, Ok(Step::Event(_) | Step::Unnamed));

      if told.send(step).is_err() || last {
        return;
      }
    }
  }

  fn turn(&self) -> Option<i64> {
    self.named.lock().unwrap_or_else(PoisonError::into_inner).turn
  }

  // Whether an event is of the followed session.
  fn mine(&self, event: &Event) -> bool {
    event.data.get("session").and_then(Value::as_str).is_some_and(|one| one == self.session)
  }

  // An event of the session read: the turn's id and run where it names them, its end where it is the end of the
  // turn followed, or of the only turn the session carried since the turn went out.
  fn heard(&mut self, event: &Event) -> Option<Step> {
    self.mark = event.id.or(self.mark);
    self.named.lock().unwrap_or_else(PoisonError::into_inner).mark = self.mark;

    if !self.mine(event) {
      return None;
    }

    // `turn.created` names its turn `turnId`, and `turn.ended` names it `turn`.
    let turn = ["turnId", "turn"]
      .iter()
      .find_map(|field| event.data.get(*field).and_then(Value::as_i64));

    match event.kind.as_str() {
      "turn.created" => {
        let mut named = self.named.lock().unwrap_or_else(PoisonError::into_inner);

        named.turn = turn.or(named.turn);
        named.run = event.data.get("run").and_then(Value::as_str).map(str::to_owned).or(named.run.take());

        Some(Step::Event(event.clone()))
      }
      "turn.ended" if turn.is_none() || self.turn().is_none() || turn == self.turn() => {
        Some(Step::Ended(event.data.clone()))
      }
      "turn.ended" => None,
      _ => Some(Step::Event(event.clone())),
    }
  }

  // The stream let go, read as after a cut.
  fn cut(&mut self) {
    self.stream = None;
    self.cut_since.get_or_insert_with(Instant::now);
  }

  // The next step of the turn, re-attaching across every cut.
  async fn next(&mut self) -> Result<Step, Failure> {
    loop {
      if let Some(stream) = self.stream.as_mut() {
        match stream.next().await {
          Some(Ok(event)) => {
            self.cut_since = None;

            if let Some(step) = self.heard(&event) {
              return Ok(step);
            }

            continue;
          }
          Some(Err(why)) => eprintln!("session {}: the agency's event stream failed: {why}", self.session),
          None => eprintln!("session {}: the agency's event stream ended", self.session),
        }

        self.stream = None;
        self.cut();
      }

      if let Some(step) = self.reattach().await? {
        return Ok(step);
      }
    }
  }

  // Waits one pace, or fails where the agency has not answered for the policy's while since the cut.
  async fn waited(&mut self, what: &str, why: &PortFailure) -> Result<(), Failure> {
    let since = *self.cut_since.get_or_insert_with(Instant::now);

    if since.elapsed() >= self.bound {
      return Err(Failure::Transport(format!(
        "the agency did not answer for session {}'s turn while it was re-attached to ({what}: {why})",
        self.session
      )));
    }

    tokio::time::sleep(self.pace).await;

    Ok(())
  }

  // One re-attachment: the stream opened again, the events kept since the mark read, and the turn read. Its end
  // where it ended; nothing where it runs on; word that no turn of the session was named where none was.
  async fn reattach(&mut self) -> Result<Option<Step>, Failure> {
    self.cut();

    let stream = loop {
      match self.agency.events(self.kinds).await {
        Ok(stream) => break stream,
        Err(why) => self.waited("its events", &why).await?,
      }
    };

    self.stream = Some(stream);

    // The events the agency kept since the last one heard, read page after page; a page that cannot be read is
    // asked again while the agency is not there, and passed over where the agency answered it otherwise.
    loop {
      let page = match self.agency.kept_events(self.mark).await {
        Ok(page) => page,
        Err(why) if unreachable(&why) => {
          self.waited("its kept events", &why).await?;
          continue;
        }
        Err(why) => {
          eprintln!("session {}: the events the agency kept were not read: {why}", self.session);
          break;
        }
      };
      let more = page.more && !page.items.is_empty();

      for event in page.items {
        if let Some(step @ Step::Ended(_)) = self.heard(&event) {
          self.cut_since = None;

          return Ok(Some(step));
        }
      }

      if !more {
        break;
      }
    }

    let Some(turn) = self.turn() else {
      self.cut_since = None;

      return Ok(Some(Step::Unnamed));
    };

    loop {
      match self.agency.turn_read(&self.session, turn).await {
        // A read that carries the turn's end gives it.
        Ok(read) if read.ended() && read.outcome.is_some() => {
          self.cut_since = None;

          return Ok(Some(Step::Ended(json!({
            "session": self.session,
            "turnId": turn,
            "outcome": read.outcome,
            "answer": read.answer,
            "reason": read.reason,
            "delivery": read.delivery,
          }))));
        }
        // The read says the turn ended and carries no end of it; its end is the event's, which the agency no
        // longer keeps. A turn whose answer was never read fails, uncounted, rather than be taken as answered.
        Ok(read) if read.ended() => {
          return Err(Failure::Transport(format!(
            "session {}'s turn {turn} ended with no end the agency still keeps",
            self.session
          )));
        }
        Ok(_) => {
          self.cut_since = None;
          println!("session {}: re-attached to turn {turn}, which runs on", self.session);

          return Ok(None);
        }
        Err(why) if unreachable(&why) => self.waited("its turn", &why).await?,
        Err(why) => {
          return Err(Failure::Transport(format!(
            "the agency holds no record of session {}'s turn {turn}: {why}",
            self.session
          )));
        }
      }
    }
  }
}

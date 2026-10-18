//! The turn a call of the receiver's tool is made in, read afresh from the call's session record (docs/chat-mcp.md,
//! 「Turn context」), and the refusal of a turn its agent's reset overtook (the agency's `docs/companions.md`, 「What the reset
//! cuts off」): a turn begun at or before the agent's reset point does nothing more, whichever tool it calls.
//! The tool host hands the turn's variables whole — its own run's, overlaid by the run a resident program's
//! place names — and the agency, which owns the resets, answers the point.

use std::collections::BTreeMap;

use receiver_ports::agency::Agency;

/// The turn variable naming the agent the turn is.
pub const AGENT_VARIABLE: &str = "BMO_TURN_AGENT";

/// The turn variable carrying the moment the turn took its words, in unix milliseconds.
pub const BEGAN_VARIABLE: &str = "BMO_TURN_BEGAN";

/// The turn variable naming the room the turn answers.
pub const CHAT_VARIABLE: &str = "BMO_TURN_CHAT";

/// The turn variable naming the road the turn answers its room on.
pub const VIA_VARIABLE: &str = "BMO_TURN_VIA";

/// The turn variable naming the messages the turn answers, by their ids, separated by commas.
pub const MESSAGES_VARIABLE: &str = "BMO_TURN_MESSAGES";

/// The variable naming the run the turn keeps its records under.
pub const RUN_NAME_VARIABLE: &str = "BMO_RUN_NAME";

/// The job runner's own marker, set once it holds its checkout's lock: a run carrying it is a job's.
pub const JOB_VARIABLE: &str = "BMO_TASK_LOCKED";

/// What a call that needs a room and reads none is refused with.
pub const NO_ROOM: &str =
  "this call was made outside any room's turn, so it has no room to act in: name one with chat";

/// What a call that is one agent's and reads no agent is refused with.
pub const NO_AGENT: &str =
  "this call was made outside any agent's turn, so there is no agent whose memory it would be";

/// Where an agent's reset point is kept among the archive's settings: the moment its last reset cut it off at,
/// in unix milliseconds.
pub fn reset_point_key(agent: &str) -> String {
  format!("reset_point:{agent}")
}

/// Whether the point overtakes a turn begun at `began`.
pub fn overtakes(point: Option<i64>, began: i64) -> bool {
  point.is_some_and(|point| began <= point)
}

/// The words a call of an overtaken turn is refused with.
pub fn overtaken_refusal(agent: &str) -> String {
  format!(
    "{agent} was reset after this turn began, so nothing the turn does is kept any more: end the turn without another call"
  )
}

/// What a call reads of its turn: the agent it is, the moment it took its words, the room it answers and the road
/// it answers on, the messages it answers, the run it keeps its records under, and whether a job runs it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Turn {
  pub agent: Option<String>,
  /// In unix milliseconds.
  pub began: Option<i64>,
  pub chat: Option<i64>,
  pub via: Option<String>,
  pub messages: Vec<i64>,
  pub run_name: Option<String>,
  pub job: bool,
}

/// The room a call acts in, and the road it goes out on where the turn names one for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
  pub chat: i64,
  pub via: Option<String>,
}

impl Turn {
  /// The turn the record's call is made in, out of the variables the tool host hands the call.
  pub fn of(variables: &BTreeMap<String, String>) -> Self {
    let named = |name: &str| -> Option<String> {
      variables
        .get(name)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    };

    Self {
      agent: named(AGENT_VARIABLE),
      began: named(BEGAN_VARIABLE).and_then(|at| at.trim().parse().ok()),
      chat: named(CHAT_VARIABLE).and_then(|chat| chat.trim().parse().ok()),
      via: named(VIA_VARIABLE),
      messages: named(MESSAGES_VARIABLE)
        .unwrap_or_default()
        .split(',')
        .filter_map(|one| one.trim().parse().ok())
        .collect(),
      run_name: named(RUN_NAME_VARIABLE),
      job: named(JOB_VARIABLE).is_some(),
    }
  }

  /// The room a call acts in: the one it named, or the turn's own. The turn's road goes with the turn's own room
  /// alone; a call addressed to another room goes out on that room's own road.
  pub fn room(&self, named: Option<i64>) -> Option<Room> {
    let chat = named.or(self.chat)?;
    let own = named.is_none() || named == self.chat;

    Some(Room {
      chat,
      via: if own { self.via.clone() } else { None },
    })
  }

  /// The refusal of this turn where its agent's reset overtook it, the point read from the agency; none for
  /// a turn naming no agent or no moment, or where no reset point can be read.
  pub async fn overtaken(&self, agency: &dyn Agency) -> Option<String> {
    let (Some(agent), Some(began)) = (&self.agent, self.began) else {
      return None;
    };

    overtakes(reset_point(agency, agent).await, began).then(|| overtaken_refusal(agent))
  }
}

// The agent's reset point, where it was ever reset and the agency answers.
async fn reset_point(agency: &dyn Agency, agent: &str) -> Option<i64> {
  agency.reset_point(agent).await.ok().flatten()
}

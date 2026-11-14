//! The files the process holds open, watched against the ceiling the system sets for it (docs/receiver.md, 「The
//! open-file ceiling」): counted on the policy's tick, and said in the journal once as the count nears the ceiling
//! and once as it falls back under.

use std::time::Duration;

use receiver_policy::open_files::OPEN_FILES;
use rustix::process::{Resource, getrlimit};
use tokio::time::MissedTickBehavior;

/// Where the system lists the files a process holds open, one entry a file.
const HELD: &str = "/proc/self/fd";

/// Whether the count stands near the ceiling, so each crossing is said once.
#[derive(Debug)]
pub struct Watch {
  near_percent: u64,
  near: bool,
}

impl Watch {
  /// A watch saying when the count reaches `near_percent` of the ceiling.
  pub fn new(near_percent: u64) -> Self {
    Self {
      near_percent,
      near: false,
    }
  }

  /// The line one reading says: one as `open` first reaches the watch's share of `ceiling`, one as it first falls
  /// back under, and none while it stays on the side it stood.
  pub fn read(&mut self, open: u64, ceiling: u64) -> Option<String> {
    let near = open.saturating_mul(100) >= ceiling.saturating_mul(self.near_percent);

    if near == self.near {
      return None;
    }

    self.near = near;

    Some(if near {
      format!(
        "bmo-receiver: {open} files stand open against the process's ceiling of {ceiling}; at the ceiling every call \
         the receiver makes and every connection it takes fails with too many open files"
      )
    } else {
      format!(
        "bmo-receiver: {open} files stand open again, under {}% of the process's ceiling of {ceiling}",
        self.near_percent
      )
    })
  }
}

/// How many files the process holds open; none where the system does not list them.
pub fn held() -> Option<u64> {
  let entries = std::fs::read_dir(HELD).ok()?;

  // The listing holds a file of its own open while it is read.
  Some((entries.count() as u64).saturating_sub(1))
}

/// The ceiling the system sets on the files the process holds open; none where it sets none.
pub fn ceiling() -> Option<u64> {
  getrlimit(Resource::Nofile).current
}

/// The watch kept for the process's life on the policy's tick; it stops where the count or the ceiling cannot be
/// read.
pub fn keep() {
  let every = Duration::from_secs(OPEN_FILES.check_seconds);

  tokio::spawn(async move {
    let mut watch = Watch::new(OPEN_FILES.near_percent);
    let mut tick = tokio::time::interval(every);

    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
      tick.tick().await;

      let (Some(open), Some(ceiling)) = (held(), ceiling()) else { return };

      if let Some(line) = watch.read(open, ceiling) {
        eprintln!("{line}");
      }
    }
  });
}

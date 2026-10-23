//! A file the owner put into a room on the console (docs/receiver.md, 「The owner's words」), and the ceiling it is
//! turned away past before anything is kept.

use receiver_policy::files::FILES;

/// A file the owner put in on the screen: its name, its type where the browser gave one and its bytes.
#[derive(Clone, Debug)]
pub struct OwnerFile {
  pub name: String,
  pub mime: String,
  pub bytes: Vec<u8>,
}

/// Why a file is turned away before anything is kept: past the ceiling every road takes a file at.
pub fn refusal(name: &str, bytes: u64) -> Option<String> {
  (bytes > FILES.largest_bytes)
    .then(|| format!("{name} is {bytes} bytes, past the {} a road takes a file at", FILES.largest_bytes))
}

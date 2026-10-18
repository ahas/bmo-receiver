//! The web unit as a placed tool reaches it (docs/boundary-api.md, 「The web unit」): whether a sender holds a
//! relation in a room, asked over the unit's own port, which a placed tool finds as the tool host finds it — by
//! the variables naming it, or under the data root in the env file the unit writes as it is deployed.

use std::path::Path;

use receiver_ports::http::Peer;
use receiver_ports::web::HttpWeb;
use receiver_ports::web::Web;

use crate::bot::env_settings;

/// The variable naming the web unit's base address.
pub const ADDRESS_VARIABLE: &str = "WEB_ADDRESS";

/// The variable naming the bearer token the web unit takes.
pub const TOKEN_VARIABLE: &str = "WEB_TOKEN";

/// The env file under the data root the web unit writes its address and token into.
pub const ENV_FILE: &str = "env/web.env";

/// The web unit a data root belongs to: where it answers and the token it takes, each variable this process
/// holds standing first.
pub fn peer(data_root: &Path) -> Peer {
  let file = env_settings(&data_root.join(ENV_FILE));
  let named = |name: &str| -> String {
    std::env::var(name)
      .ok()
      .or_else(|| file.get(name).cloned())
      .map(|value| value.trim().to_owned())
      .filter(|value| !value.is_empty())
      .unwrap_or_default()
  };

  Peer {
    base: named(ADDRESS_VARIABLE),
    token: named(TOKEN_VARIABLE),
  }
}

/// Whether a sender holds a relation in a room, as the web unit answers it, or why it did not answer. No
/// sender at all is checked as the room's members alone.
pub async fn holds(
  data_root: &Path,
  room: i64,
  sender: Option<i64>,
  relation: &str,
) -> Result<bool, String> {
  let web = HttpWeb::new(&peer(data_root));
  let check = receiver_ports::web::RoomCheck {
    room,
    sender,
    relation: relation.to_owned(),
  };
  let checked = web.check_room(&check).await.map_err(|why| why.to_string())?;

  Ok(checked.allowed)
}

//! The receiver's configuration (docs/boundary-api.md, 「Configuration」): the one TOML file the binary reads at
//! start and holds for the process's life — its listen address, its token, its database, the deployment's names,
//! and each unit it calls with that unit's base address and token. Nothing a deployment chooses stands in the code.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use receiver_ports::agency::HttpAgency;
use receiver_ports::brain::HttpBrain;
use receiver_ports::chat::ChatServices;
use receiver_ports::gate::HttpGate;
use receiver_ports::http::Peer;
use receiver_ports::job::HttpJob;
use receiver_ports::voice::HttpVoice;
use receiver_ports::warmer::HttpWarmer;
use receiver_ports::web::HttpWeb;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use serde::Deserialize;

use crate::data_root::DataRoot;
use crate::receiver::{Deployment, Receiver, Units};
use crate::roads::Roads;
use crate::verbs::reload::Places;

/// The consumer the receiver registers with the cache warmer, unless the configuration names another.
const CONSUMER: &str = "receiver";

/// The configuration file's own name.
pub const SETTINGS: &str = "settings.toml";

/// The variable naming the folder a release's configuration stands in.
pub const SETTINGS_FOLDER: &str = "BMO_RECEIVER_SETTINGS";

/// The folder under the data root the brain's documents stand in, unless the configuration names another root.
const BRAIN: &str = "brain";

/// The configuration, key by key as the file writes it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
  pub listen: String,
  pub token: String,
  pub database: String,
  pub data_root: PathBuf,
  #[serde(default = "consumer_name")]
  pub consumer: String,
  pub owner: i64,
  pub agent: String,
  pub sender_name: String,
  #[serde(default)]
  pub delegate: Option<String>,
  #[serde(default)]
  pub infra_room: Option<i64>,
  pub language: String,
  #[serde(default)]
  pub attachment_bytes: Option<u64>,
  #[serde(default)]
  pub wording_document: Option<PathBuf>,
  #[serde(default)]
  pub brain_root: Option<PathBuf>,
  #[serde(default)]
  pub document_tree: Option<PathBuf>,
  pub agency: Peer,
  pub brain: Peer,
  pub gate: Peer,
  pub job: Peer,
  pub voice: Peer,
  pub web: Peer,
  pub warmer: Peer,
  #[serde(default)]
  pub roads: BTreeMap<String, Peer>,
}

/// The consumer's name where the configuration leaves it out.
fn consumer_name() -> String {
  CONSUMER.to_owned()
}

/// The configuration file this process reads: the one `--config` names, else `settings.toml` in the folder
/// `BMO_RECEIVER_SETTINGS` names, else in the folder above the binary's own — a release's root — else in this
/// directory.
pub fn path(explicit: Option<&Path>) -> PathBuf {
  if let Some(explicit) = explicit {
    return explicit.to_path_buf();
  }

  let named = std::env::var_os(SETTINGS_FOLDER).filter(|folder| !folder.is_empty()).map(PathBuf::from);
  let release = || std::env::current_exe().ok()?.parent()?.parent().map(Path::to_path_buf);

  named.or_else(release).unwrap_or_else(|| PathBuf::from(".")).join(SETTINGS)
}

impl Config {
  /// The configuration a file holds, or why it could not be read.
  pub fn read(path: &Path) -> Result<Self, String> {
    let text = std::fs::read_to_string(path).map_err(|why| format!("{}: {why}", path.display()))?;

    toml::from_str(&text).map_err(|why| format!("{}: {why}", path.display()))
  }

  /// The receiver's database, connected.
  pub async fn connect(&self) -> Result<DatabaseConnection, String> {
    let mut options = ConnectOptions::new(self.database.clone());

    options.sqlx_logging(false);

    Database::connect(options).await.map_err(|why| format!("the database: {why}"))
  }

  /// The deployment's names, with the words a room calls the chat agent by read off its registration.
  pub fn deployment(&self, call_names: Vec<String>) -> Deployment {
    Deployment {
      owner_id: self.owner,
      agent: self.agent.clone(),
      sender_name: self.sender_name.clone(),
      addresses: call_names,
      delegate: self.delegate.clone(),
      infra_room: self.infra_room,
      language: self.language.clone(),
      attachment_bytes: self.attachment_bytes,
      wording_document: self.wording_document.clone(),
    }
  }

  /// The units a receiver reaches: the chat services through their roads, and each unit this configuration names.
  pub fn units(&self) -> Units {
    Units {
      roads: Roads::new(ChatServices::from_peers(&self.roads)),
      agency: Arc::new(HttpAgency::new(&self.agency)),
      brain: Arc::new(HttpBrain::new(&self.brain)),
      gate: Arc::new(HttpGate::new(&self.gate)),
      job: Arc::new(HttpJob::new(&self.job)),
      voice: Arc::new(HttpVoice::new(&self.voice)),
      web: Arc::new(HttpWeb::new(&self.web)),
      warmer: Arc::new(HttpWarmer::new(&self.warmer)),
    }
  }

  /// The data root the configuration names, with the stores under it.
  pub fn roots(&self) -> DataRoot {
    DataRoot::new(self.data_root.clone())
  }

  /// Where a document reload reads what it names: the tree the agents' shipped documents stand in, where the
  /// configuration names one, and the brain root, `brain` under the data root where it names none.
  pub fn places(&self) -> Places {
    Places {
      tree: self.document_tree.clone(),
      brain: self.brain_root.clone().unwrap_or_else(|| self.data_root.join(BRAIN)),
    }
  }

  /// A receiver on the database, for this configuration's deployment and units, with its database's place
  /// held for the reads that open a database by place rather than through the pool.
  pub fn receiver(&self, db: DatabaseConnection, call_names: Vec<String>) -> Receiver {
    let receiver = Receiver::new(db, self.roots(), self.deployment(call_names), self.units());
    let place: Arc<str> = self.database.clone().into();

    receiver.hold_place(place);

    receiver
  }
}

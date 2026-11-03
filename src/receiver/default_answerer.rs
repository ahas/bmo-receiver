//! The default answerer (docs/api.md, 「Supervisors」): a setting of the receiver's naming the supervisor that
//! answers every room naming none, which is also the supervisor a new room gets. The store's settings hold it; a
//! store holding none is seeded with the configuration's `agent` as the receiver starts. Every clone reads and
//! moves the one setting.

use std::sync::{Arc, PoisonError, RwLock};

use sea_orm::ConnectionTrait;

use crate::store::{Fault, settings};

/// The settings key the setting is kept under.
pub const KEY: &str = "default_answerer";

/// The setting as the process holds it: read from the store as the receiver starts and moved by the console.
#[derive(Clone, Debug)]
pub struct DefaultAnswerer(Arc<RwLock<String>>);

impl DefaultAnswerer {
  /// The setting standing on the configured name until the store is read.
  pub fn configured(name: &str) -> Self {
    Self(Arc::new(RwLock::new(name.to_owned())))
  }

  /// The supervisor's name the setting holds now.
  pub fn name(&self) -> String {
    self.0.read().unwrap_or_else(PoisonError::into_inner).clone()
  }

  /// The setting read from the store, the store seeded with the configured name where it holds none.
  pub async fn load(&self, db: &impl ConnectionTrait) -> Result<(), Fault> {
    match settings::get(db, KEY).await?.map(|name| name.trim().to_owned()) {
      Some(name) if !name.is_empty() => self.hold(&name),
      _ => settings::set(db, KEY, &self.name()).await?,
    }

    Ok(())
  }

  /// The setting moved to a supervisor's name, written to the store first.
  pub async fn set(&self, db: &impl ConnectionTrait, name: &str) -> Result<(), Fault> {
    settings::set(db, KEY, name).await?;
    self.hold(name);

    Ok(())
  }

  fn hold(&self, name: &str) {
    *self.0.write().unwrap_or_else(PoisonError::into_inner) = name.to_owned();
  }
}

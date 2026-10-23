//! What the receiver keeps of another unit's answers for a short span before it asks again (docs/receiver.md,
//! 「What is kept for a moment」): each answer under its key with the moment it was read, read back only while the
//! span has not passed since.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Answers kept by key for a span; clones share what is kept.
#[derive(Clone, Debug)]
pub struct Kept<K, V> {
  span: Duration,
  held: Arc<Mutex<BTreeMap<K, (Instant, V)>>>,
}

impl<K: Ord, V: Clone> Kept<K, V> {
  /// Nothing kept yet, each answer to be kept for `span`; a span of zero keeps nothing.
  pub fn new(span: Duration) -> Self {
    Self {
      span,
      held: Arc::default(),
    }
  }

  fn held(&self) -> std::sync::MutexGuard<'_, BTreeMap<K, (Instant, V)>> {
    self.held.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  /// The answer kept under the key, while its span has not passed.
  pub fn get(&self, key: &K) -> Option<V> {
    self
      .held()
      .get(key)
      .filter(|(read, _)| read.elapsed() < self.span)
      .map(|(_, value)| value.clone())
  }

  /// An answer kept under its key from now, in place of any before; where nothing is kept, nothing is.
  pub fn keep(&self, key: K, value: V) {
    if self.span.is_zero() {
      return;
    }

    let mut held = self.held();

    held.retain(|_, (read, _)| read.elapsed() < self.span);
    held.insert(key, (Instant::now(), value));
  }

  /// Every answer kept under a key `matches` holds for let go.
  pub fn forget(&self, matches: impl Fn(&K) -> bool) {
    self.held().retain(|key, _| !matches(key));
  }
}

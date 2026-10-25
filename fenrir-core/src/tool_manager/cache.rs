use std::collections::HashMap;
use std::sync::Mutex;

pub struct ToolCache<K, V> {
  inner: Mutex<HashMap<K, V>>,
}

impl<K, V> ToolCache<K, V>
where
  K: Eq + std::hash::Hash,
{
  pub fn new() -> Self {
    return Self {
      inner: Mutex::new(HashMap::new()),
    };
  }

  pub fn get(&self, key: &K) -> Option<V>
  where
    V: Clone,
  {
    return self
      .inner
      .lock()
      .unwrap_or_else(|e| e.into_inner())
      .get(key)
      .cloned();
  }

  pub fn insert(&self, key: K, value: V) {
    self
      .inner
      .lock()
      .unwrap_or_else(|e| e.into_inner())
      .insert(key, value);
  }

  pub fn clear(&self) {
    self.inner.lock().unwrap_or_else(|e| e.into_inner()).clear();
  }
}

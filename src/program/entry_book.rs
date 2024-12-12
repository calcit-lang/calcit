use std::{fmt::Debug, sync::Arc};

/// this is a replacement for HashMap with a vector.
/// it's optimized for faster reading with index, while searching being very slow.
/// it ensures that it index does not change, which offers oppotunity for user to cache the index value
#[derive(Debug, Clone)]
pub struct EntryBook<T>(Vec<EntryPair<T>>)
where
  T: Clone;

impl<T> Default for EntryBook<T>
where
  T: Clone,
{
  fn default() -> Self {
    EntryBook(Vec::new())
  }
}

impl<T> EntryBook<T>
where
  T: Clone,
{
  /// find entry and insert, even if it's a tombstone
  pub fn insert(&mut self, key: Arc<str>, value: T) {
    for piece in self.0.iter_mut() {
      // println!("comparing {} and {}", &*piece.key, key);
      if piece.key == key {
        *piece = EntryPair { key, value: Some(value) };

        return;
      }
    }

    self.0.push(EntryPair { key, value: Some(value) })
  }

  pub fn len(&self) -> usize {
    self.0.len()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// O(n) search, search from start to end
  pub fn lookup(&self, key: &str) -> Option<(&T, u16)> {
    // println!("searching {} in keys {:?}", key, self.read_keys());
    for (idx, piece) in self.0.iter().enumerate() {
      if &*piece.key == key {
        match &piece.value {
          Some(v) => {
            // println!("found {} at {}", key, idx);
            return Some((v, idx as u16));
          }
          None => continue,
        }
      }
    }
    // println!("not found {}", key);
    None
  }

  pub fn read_keys(&self) -> Vec<Arc<str>> {
    self.0.iter().map(|piece| piece.key.to_owned()).collect()
  }

  pub fn load(&self, idx: u16) -> (&T, &str) {
    match self.0.get(idx as usize) {
      Some(piece) => match &piece.value {
        Some(v) => (v, &*piece.key),
        None => unreachable!("given index {} is invalid in current book of size {}", idx, self.0.len()),
      },
      None => unreachable!("given index {} is invalid in current book of size {}", idx, self.0.len()),
    }
  }

  pub fn check_name(&self, idx: usize, key: &str) -> Result<(), String> {
    match self.0.get(idx) {
      Some(piece) => {
        if &*piece.key == key {
          Ok(())
        } else {
          Err(format!("index {} is not for key {}", idx, key))
        }
      }
      None => Err(format!("index {} is invalid in current book of size {}", idx, self.0.len())),
    }
  }

  /// remove all data
  pub fn clear(&mut self) {
    self.0.clear();
  }

  /// TODO need better iterator
  pub fn iter(&self) -> impl Iterator<Item = (Arc<str>, &T)> {
    self
      .0
      .iter()
      .filter_map(|piece| piece.value.as_ref().map(|v| (piece.key.to_owned(), v)))
  }

  pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
    match self.0.get_mut(idx) {
      Some(piece) => match &mut piece.value {
        Some(v) => Some(v),
        None => None,
      },
      None => None,
    }
  }

  pub fn lookup_mut(&mut self, key: &str) -> Option<(&mut T, u16)> {
    for (idx, piece) in self.0.iter_mut().enumerate() {
      // println!("comparing {} and {}", &*piece.key, key);
      if &*piece.key == key {
        match &mut piece.value {
          Some(v) => return Some((v, idx as u16)),
          None => return None,
        }
      }
    }
    None
  }

  /// can not really remove key, use a default value as tombstone
  pub fn remove(&mut self, key: &str) {
    for piece in self.0.iter_mut() {
      if &*piece.key == key {
        piece.value = None;
      }
    }
  }

  pub fn keys(&self) -> impl Iterator<Item = &Arc<str>> {
    self.0.iter().map(|x| &x.key)
  }

  pub fn to_hashmap(&self) -> std::collections::HashMap<Arc<str>, T> {
    let mut res = std::collections::HashMap::with_capacity(self.0.len());
    for piece in &self.0 {
      if let Some(v) = &piece.value {
        res.insert(piece.key.to_owned(), v.to_owned());
      }
    }
    res
  }
}

#[derive(Debug, Clone)]
pub struct EntryPair<T>
where
  T: Clone,
{
  pub key: Arc<str>,
  pub value: Option<T>,
}

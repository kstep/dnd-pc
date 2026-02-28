use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct VecSet<T>(Vec<T>);

impl<T> Default for VecSet<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: PartialEq> VecSet<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Push if not already present. Returns `true` if inserted.
    pub fn insert(&mut self, value: T) -> bool {
        if self.0.contains(&value) {
            false
        } else {
            self.0.push(value);
            true
        }
    }

    /// Remove first match. Returns `true` if found.
    pub fn remove(&mut self, value: &T) -> bool {
        if let Some(pos) = self.0.iter().position(|v| v == value) {
            self.0.remove(pos);
            true
        } else {
            false
        }
    }
}

impl<T> VecSet<T> {
    /// Unconditional push — for UI "add empty placeholder" pattern.
    pub fn push(&mut self, value: T) {
        self.0.push(value);
    }

    /// Replace value at index.
    pub fn set(&mut self, index: usize, value: T) {
        self.0[index] = value;
    }

    /// Remove by index (like `Vec::remove`).
    pub fn remove_at(&mut self, index: usize) -> T {
        self.0.remove(index)
    }
}

impl<T> Deref for VecSet<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<T: PartialEq> From<Vec<T>> for VecSet<T> {
    fn from(vec: Vec<T>) -> Self {
        let mut set = Self(Vec::with_capacity(vec.len()));
        set.extend(vec);
        set
    }
}

impl<T> IntoIterator for VecSet<T> {
    type IntoIter = std::vec::IntoIter<T>;
    type Item = T;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a VecSet<T> {
    type IntoIter = std::slice::Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: PartialEq> Extend<T> for VecSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.insert(item);
        }
    }
}

impl<T: PartialEq> FromIterator<T> for VecSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::new();
        set.extend(iter);
        set
    }
}

impl<'de, T: Deserialize<'de> + PartialEq> Deserialize<'de> for VecSet<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let vec = Vec::<T>::deserialize(deserializer)?;
        Ok(Self::from(vec))
    }
}

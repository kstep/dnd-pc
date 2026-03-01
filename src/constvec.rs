use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize, Serializer, de};

/// A fixed-size vector, which serializes/deserializes to the last non-zero
/// portion of the vector for compactness.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ConstVec<T, const N: usize> {
    data: [T; N],
}

impl<'de, T, const N: usize> Deserialize<'de> for ConstVec<T, N>
where
    T: PartialEq + Default + Copy + Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vec = Vec::<T>::deserialize(deserializer)?;
        if vec.len() > N {
            return Err(de::Error::custom(format!(
                "Expected at most {N} elements, got {}",
                vec.len()
            )));
        }
        let mut data = [T::default(); N];
        data[..vec.len()].copy_from_slice(&vec);
        Ok(Self { data })
    }
}

impl<T, const N: usize> Serialize for ConstVec<T, N>
where
    T: PartialEq + Default + Copy + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_slice().serialize(serializer)
    }
}

impl<T, const N: usize> Deref for ConstVec<T, N>
where
    T: PartialEq + Default + Copy,
{
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T, const N: usize> DerefMut for ConstVec<T, N>
where
    T: PartialEq + Default + Copy,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T, const N: usize> ConstVec<T, N>
where
    T: PartialEq + Default + Copy,
{
    pub fn new() -> Self {
        Self {
            data: [T::default(); N],
        }
    }

    pub fn max_len(&self) -> usize {
        N
    }

    pub fn len(&self) -> usize {
        let zero = T::default();
        self.data
            .iter()
            .rposition(|x| *x != zero)
            .map_or(0, |pos| pos + 1)
    }

    pub fn as_slice(&self) -> &[T] {
        &self.data[..self.len()]
    }
}

impl<T, const N: usize> Default for ConstVec<T, N>
where
    T: PartialEq + Default + Copy,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> AsRef<[T]> for ConstVec<T, N>
where
    T: PartialEq + Default + Copy,
{
    fn as_ref(&self) -> &[T] {
        &self.data[..]
    }
}

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Die {
    pub amount: u32,
    pub sides: u32,
}

impl fmt::Display for Die {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}d{}", self.amount, self.sides)
    }
}

impl FromStr for Die {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (amount, sides) = s.split_once('d').ok_or("expected {amount}d{sides}")?;
        Ok(Die {
            amount: amount.parse().map_err(|_| "invalid amount")?,
            sides: sides.parse().map_err(|_| "invalid sides")?,
        })
    }
}

impl Serialize for Die {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Die {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

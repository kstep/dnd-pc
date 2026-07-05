use std::{
    fmt,
    iter::Sum,
    ops::{Add, Mul},
    str::FromStr,
};

use reactive_stores::Store;
use serde::{Deserialize, Serialize};

/// Item weight in hundredths of a pound (25 = 0.25 lb).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    Serialize,
    Deserialize,
    Store
)]
#[serde(transparent)]
pub struct Weight(pub u32);

impl Weight {
    pub fn from_hundredths(hundredths: u32) -> Self {
        Self(hundredths)
    }
}

impl Mul<u32> for Weight {
    type Output = Self;

    fn mul(self, quantity: u32) -> Self {
        Self(self.0.saturating_mul(quantity))
    }
}

impl Add for Weight {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
}

impl Sum for Weight {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

impl fmt::Display for Weight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.0 / 100;
        let frac = self.0 % 100;
        if frac == 0 {
            write!(f, "{whole}")
        } else if frac.is_multiple_of(10) {
            write!(f, "{whole}.{}", frac / 10)
        } else {
            write!(f, "{whole}.{frac:02}")
        }
    }
}

impl FromStr for Weight {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (whole_str, frac_str) = s.split_once('.').unwrap_or((s, ""));
        let whole = whole_str.parse::<u32>().map_err(|_| ())?;
        let frac_str = frac_str.trim();
        let frac = match frac_str.len() {
            0 => 0,
            1 => frac_str.parse::<u32>().map_err(|_| ())? * 10,
            _ => frac_str[..2].parse::<u32>().map_err(|_| ())?,
        };
        Ok(Self(whole * 100 + frac))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_display_trims_zeros() {
        assert_eq!(Weight(25).to_string(), "0.25");
        assert_eq!(Weight(150).to_string(), "1.5");
        assert_eq!(Weight(300).to_string(), "3");
        assert_eq!(Weight(0).to_string(), "0");
    }

    #[test]
    fn weight_parses_decimal() {
        assert_eq!("0.25".parse::<Weight>().unwrap(), Weight(25));
        assert_eq!("1.5".parse::<Weight>().unwrap(), Weight(150));
        assert_eq!("3".parse::<Weight>().unwrap(), Weight(300));
        assert_eq!("1.05".parse::<Weight>().unwrap(), Weight(105));
        assert!("".parse::<Weight>().is_err());
        assert!("-1".parse::<Weight>().is_err());
        assert!("abc".parse::<Weight>().is_err());
    }

    #[test]
    fn weight_totals() {
        assert_eq!(Weight(150) * 4, Weight(600));
        let total: Weight = [Weight(25), Weight(150)].into_iter().sum();
        assert_eq!(total, Weight(175));
    }

    #[test]
    fn weight_serde_transparent() {
        let json = serde_json::to_string(&Weight(150)).unwrap();
        assert_eq!(json, "150");
        assert_eq!(serde_json::from_str::<Weight>("150").unwrap(), Weight(150));
    }
}

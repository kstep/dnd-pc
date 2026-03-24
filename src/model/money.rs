use std::{
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Money {
    cp: u32,
}

impl Money {
    pub const CP_PER_EP: u32 = 50;
    pub const CP_PER_GP: u32 = 100;
    pub const CP_PER_PP: u32 = 1000;
    pub const CP_PER_SP: u32 = 10;

    pub fn from_gp_str(gp_str: &str) -> Option<Self> {
        let gp_str = gp_str.trim();
        if gp_str.is_empty() {
            return Some(Self::default());
        }

        let (whole_gp_str, fraction_gp_str) = gp_str.split_once('.').unwrap_or((gp_str, ""));
        let whole_gp = whole_gp_str.trim().parse::<u32>().ok()?;

        let fraction_gp_str = fraction_gp_str.trim();
        let fraction_gp_str = if fraction_gp_str.len() > 2 {
            &fraction_gp_str[..2]
        } else {
            fraction_gp_str
        };

        if fraction_gp_str.is_empty() {
            return Some(Self::from_gp(whole_gp));
        }

        let fraction_gp = fraction_gp_str.parse::<u32>().ok()?;
        let fraction_gp = if fraction_gp_str.len() == 1 {
            // "0.5" should be treated as "0.50"
            fraction_gp * 10
        } else {
            fraction_gp
        };

        Some(Self::from_gp_cp(whole_gp, fraction_gp))
    }

    pub fn from_cp(cp: u32) -> Self {
        Self { cp }
    }

    pub fn from_gp(gp: u32) -> Self {
        Self {
            cp: gp * Self::CP_PER_GP,
        }
    }

    pub fn from_gp_cp(gp: u32, cp: u32) -> Self {
        Self {
            cp: gp * Self::CP_PER_GP + cp,
        }
    }

    // gp, sp, cp
    pub fn as_gp_sp_cp(&self) -> (u32, u32, u32) {
        let mut remaining_cp = self.cp;

        let gp = remaining_cp / Self::CP_PER_GP;
        remaining_cp %= Self::CP_PER_GP;

        let sp = remaining_cp / Self::CP_PER_SP;
        remaining_cp %= Self::CP_PER_SP;

        let cp = remaining_cp;

        (gp, sp, cp)
    }

    pub fn whole_cp(&self) -> u32 {
        self.cp
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut remaining_cp = self.cp;

        macro_rules! write_part {
            ($f:ident, $part:ident, $suffix:expr, $remaining:ident) => {{
                let amount = $remaining / Self::$part;
                $remaining %= Self::$part;
                if amount > 0 {
                    write!($f, "{amount}{}", $suffix)?;
                    if $remaining > 0 {
                        $f.write_str(" ")?;
                    }
                }
            }};
        }

        write_part!(f, CP_PER_PP, "pp", remaining_cp);
        write_part!(f, CP_PER_GP, "gp", remaining_cp);
        write_part!(f, CP_PER_EP, "ep", remaining_cp);
        write_part!(f, CP_PER_SP, "sp", remaining_cp);

        if remaining_cp > 0 {
            write!(f, "{remaining_cp}cp")?;
        }

        Ok(())
    }
}

impl Add for Money {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            cp: self.cp + rhs.cp,
        }
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Self) {
        self.cp += rhs.cp;
    }
}

impl Sub for Money {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            cp: self.cp - rhs.cp,
        }
    }
}

impl SubAssign for Money {
    fn sub_assign(&mut self, rhs: Self) {
        self.cp -= rhs.cp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_constructors() {
        assert_eq!(Money::from_cp(1).whole_cp(), 1);
        assert_eq!(Money::from_gp(1).whole_cp(), 100);
    }

    #[test]
    fn from_gp_cp() {
        assert_eq!(Money::from_gp_cp(2, 50).whole_cp(), 250);
        assert_eq!(Money::from_gp_cp(0, 0).whole_cp(), 0);
    }

    #[test]
    fn as_gp_sp_cp_roundtrip() {
        let money = Money::from_cp(1234);
        let (gp, sp, cp) = money.as_gp_sp_cp();
        assert_eq!(gp, 12);
        assert_eq!(sp, 3);
        assert_eq!(cp, 4);
    }

    #[test]
    fn as_gp_sp_cp_exact_denominations() {
        assert_eq!(Money::from_gp(5).as_gp_sp_cp(), (5, 0, 0));
        assert_eq!(Money::from_cp(70).as_gp_sp_cp(), (0, 7, 0));
        assert_eq!(Money::from_cp(9).as_gp_sp_cp(), (0, 0, 9));
    }

    #[test]
    fn from_gp_str_whole() {
        assert_eq!(Money::from_gp_str("10"), Some(Money::from_gp(10)));
        assert_eq!(Money::from_gp_str("0"), Some(Money::default()));
    }

    #[test]
    fn from_gp_str_decimal() {
        // "10.50" = 10gp 50cp
        assert_eq!(Money::from_gp_str("10.50"), Some(Money::from_gp_cp(10, 50)));
        // "10.5" = 10gp 50cp (single digit treated as tens)
        assert_eq!(Money::from_gp_str("10.5"), Some(Money::from_gp_cp(10, 50)));
        // "0.01" = 1cp
        assert_eq!(Money::from_gp_str("0.01"), Some(Money::from_cp(1)));
        // "0.05" = 5cp (leading zero must not be lost)
        assert_eq!(Money::from_gp_str("0.05"), Some(Money::from_cp(5)));
        // "0.99" = 99cp
        assert_eq!(Money::from_gp_str("0.99"), Some(Money::from_cp(99)));
    }

    #[test]
    fn from_gp_str_truncates_fraction() {
        // More than 2 decimal digits: truncated to 2
        assert_eq!(Money::from_gp_str("1.999"), Some(Money::from_gp_cp(1, 99)));
    }

    #[test]
    fn from_gp_str_whitespace() {
        assert_eq!(Money::from_gp_str("  5  "), Some(Money::from_gp(5)));
        assert_eq!(Money::from_gp_str(""), Some(Money::default()));
        assert_eq!(Money::from_gp_str("  "), Some(Money::default()));
    }

    #[test]
    fn from_gp_str_invalid() {
        assert_eq!(Money::from_gp_str("abc"), None);
        assert_eq!(Money::from_gp_str("1.ab"), None);
        assert_eq!(Money::from_gp_str("-5"), None);
    }

    #[test]
    fn display_mixed() {
        assert_eq!(Money::from_cp(1234).to_string(), "1pp 2gp 3sp 4cp");
        assert_eq!(Money::from_gp(5).to_string(), "5gp");
        assert_eq!(Money::from_cp(0).to_string(), "");
        assert_eq!(Money::from_cp(3).to_string(), "3cp");
        assert_eq!(Money::from_cp(1050).to_string(), "1pp 1ep");
    }

    #[test]
    fn add_and_sub() {
        let a = Money::from_gp(5);
        let b = Money::from_gp(3);
        assert_eq!(a + b, Money::from_gp(8));
        assert_eq!(a - b, Money::from_gp(2));
    }

    #[test]
    fn add_assign_and_sub_assign() {
        let mut m = Money::from_gp(10);
        m += Money::from_gp(5);
        assert_eq!(m, Money::from_gp(15));
        m -= Money::from_gp(3);
        assert_eq!(m, Money::from_gp(12));
    }

    #[test]
    fn ordering() {
        assert!(Money::from_gp(5) > Money::from_gp(3));
        assert!(Money::from_cp(99) < Money::from_gp(1));
        assert_eq!(Money::from_cp(100), Money::from_gp(1));
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(Money::default().whole_cp(), 0);
        assert_eq!(Money::default().as_gp_sp_cp(), (0, 0, 0));
    }
}

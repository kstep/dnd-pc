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

        let fraction_gp = fraction_gp_str.parse::<u32>().ok()?;
        let fraction_gp = if fraction_gp < 10 {
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

    pub fn from_sp(sp: u32) -> Self {
        Self {
            cp: sp * Self::CP_PER_SP,
        }
    }

    pub fn from_ep(ep: u32) -> Self {
        Self {
            cp: ep * Self::CP_PER_EP,
        }
    }

    pub fn from_gp(gp: u32) -> Self {
        Self {
            cp: gp * Self::CP_PER_GP,
        }
    }

    pub fn from_pp(pp: u32) -> Self {
        Self {
            cp: pp * Self::CP_PER_PP,
        }
    }

    pub fn from_gp_cp(gp: u32, cp: u32) -> Self {
        Self {
            cp: gp * Self::CP_PER_GP + cp,
        }
    }

    // pp, gp, ep, sp, cp
    pub fn as_coins(&self) -> (u32, u32, u32, u32, u32) {
        let mut remaining_cp = self.cp;

        let pp = remaining_cp / Self::CP_PER_PP;
        remaining_cp %= Self::CP_PER_PP;

        let gp = remaining_cp / Self::CP_PER_GP;
        remaining_cp %= Self::CP_PER_GP;

        let ep = remaining_cp / Self::CP_PER_EP;
        remaining_cp %= Self::CP_PER_EP;

        let sp = remaining_cp / Self::CP_PER_SP;
        remaining_cp %= Self::CP_PER_SP;

        let cp = remaining_cp;

        (pp, gp, ep, sp, cp)
    }

    pub fn whole_cp(&self) -> u32 {
        self.cp
    }

    pub fn whole_sp(&self) -> u32 {
        self.cp / Self::CP_PER_SP
    }

    pub fn whole_ep(&self) -> u32 {
        self.cp / Self::CP_PER_EP
    }

    pub fn whole_gp(&self) -> u32 {
        self.cp / Self::CP_PER_GP
    }

    pub fn whole_pp(&self) -> u32 {
        self.cp / Self::CP_PER_PP
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

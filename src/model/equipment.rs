use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::model::{ArmorType, DamageType, Money};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Equipment {
    #[serde(default)]
    pub weapons: Vec<Weapon>,
    #[serde(default)]
    pub armors: Vec<Armor>,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Armor {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub base_ac: u32,
    #[serde(default)]
    pub armor_type: ArmorType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Weapon {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub attack_bonus: i32,
    #[serde(default)]
    pub damage: String,
    #[serde(default)]
    pub damage_type: Option<DamageType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Item {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub quantity: u32,
    #[serde(default)]
    pub description: String,
}

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if self.quantity > 1 {
            write!(f, " \u{00d7}{}", self.quantity)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Currency {
    #[serde(default)]
    pub cp: u32,
    #[serde(default)]
    pub sp: u32,
    #[serde(default)]
    pub ep: u32,
    #[serde(default)]
    pub gp: u32,
    #[serde(default)]
    pub pp: u32,
}

impl Currency {
    pub fn as_money(&self) -> Money {
        Money::from_cp(
            self.cp
                + self.sp * Money::CP_PER_SP
                + self.ep * Money::CP_PER_EP
                + self.gp * Money::CP_PER_GP
                + self.pp * Money::CP_PER_PP,
        )
    }

    pub fn gain(&mut self, amount: Money) {
        let (gain_gp, gain_sp, gain_cp) = amount.as_gp_sp_cp();
        self.cp += gain_cp;
        self.sp += gain_sp;
        self.gp += gain_gp;
    }

    #[allow(unused_assignments)]
    pub fn spend(&mut self, amount: Money) -> bool {
        if amount > self.as_money() {
            return false;
        }

        let mut remaining_cp = amount.whole_cp();

        macro_rules! spend_coin {
            ($coin:ident, $cp_per:expr) => {
                if remaining_cp > 0 {
                    let can_spend = (remaining_cp / $cp_per).min(self.$coin);
                    self.$coin -= can_spend;
                    remaining_cp -= can_spend * $cp_per;
                }
            };
        }

        spend_coin!(pp, Money::CP_PER_PP);
        spend_coin!(gp, Money::CP_PER_GP);
        spend_coin!(ep, Money::CP_PER_EP);
        spend_coin!(sp, Money::CP_PER_SP);
        spend_coin!(cp, 1u32);

        // If there's still a remainder, break the smallest available coin that
        // covers it and give change back in GP/SP/CP (no EP to keep it clean).
        // The three guards: still something to spend, coin is in wallet, coin
        // covers the remainder (one coin is enough since the greedy pass already
        // consumed all coins whose denomination divides evenly into remaining_cp).
        if remaining_cp > 0 {
            macro_rules! break_coin {
                ($coin:ident, $cp_per:expr) => {
                    if remaining_cp > 0 && self.$coin > 0 && $cp_per >= remaining_cp {
                        self.$coin -= 1;
                        let mut change = $cp_per - remaining_cp;
                        self.gp += change / Money::CP_PER_GP;
                        change %= Money::CP_PER_GP;
                        self.sp += change / Money::CP_PER_SP;
                        self.cp += change % Money::CP_PER_SP;
                        remaining_cp = 0;
                    }
                };
            }

            break_coin!(sp, Money::CP_PER_SP);
            break_coin!(ep, Money::CP_PER_EP);
            break_coin!(gp, Money::CP_PER_GP);
            break_coin!(pp, Money::CP_PER_PP);
        }

        true
    }
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for (amount, label) in [
            (self.pp, "pp"),
            (self.gp, "gp"),
            (self.ep, "ep"),
            (self.sp, "sp"),
            (self.cp, "cp"),
        ] {
            if amount > 0 {
                if !first {
                    f.write_str(" ")?;
                }
                write!(f, "{amount}{label}")?;
                first = false;
            }
        }
        if first {
            f.write_str("\u{2014}")?;
        }
        Ok(())
    }
}

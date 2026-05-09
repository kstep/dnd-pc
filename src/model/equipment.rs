use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::{
    model::{Ability, ArmorType, DamageType, Enchantment, Expr, Money, WeaponCategory},
    rules::WhenCondition,
};

/// Stable reference to a gear slot inside `Equipment`. Used by the
/// enchantment editor, gear-actions UI, and `ItemApplyCtx` to address a
/// specific item / weapon / armor by kind and index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GearRef {
    Item(usize),
    Weapon(usize),
    Armor(usize),
}

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

impl Equipment {
    /// `OnGearActive` only fires for equipped gear; other timings run on all gear.
    pub fn assignments(
        &self,
        when: WhenCondition,
    ) -> impl Iterator<Item = (GearRef, &Expr)> + '_ {
        let active_only = matches!(when, WhenCondition::OnGearActive);
        let items = self.items.iter().enumerate().flat_map(move |(i, item)| {
            let gate = !active_only || item.is_active();
            item.magic
                .assign
                .iter()
                .filter(move |a| gate && a.when == when)
                .map(move |a| (GearRef::Item(i), &a.expr))
        });
        let weapons = self
            .weapons
            .iter()
            .enumerate()
            .flat_map(move |(i, weapon)| {
                let gate = !active_only || weapon.is_active();
                weapon
                    .magic
                    .assign
                    .iter()
                    .filter(move |a| gate && a.when == when)
                    .map(move |a| (GearRef::Weapon(i), &a.expr))
            });
        let armors = self.armors.iter().enumerate().flat_map(move |(i, armor)| {
            let gate = !active_only || armor.is_active();
            armor
                .magic
                .assign
                .iter()
                .filter(move |a| gate && a.when == when)
                .map(move |a| (GearRef::Armor(i), &a.expr))
        });
        items.chain(weapons).chain(armors)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Armor {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub base_ac: u32,
    #[serde(default)]
    pub armor_type: ArmorType,
    #[serde(default)]
    pub ac_expr: Option<Expr>,
    #[serde(default)]
    pub equipped: bool,
    #[serde(default, skip_serializing_if = "Enchantment::is_empty")]
    pub magic: Enchantment,
}

impl Armor {
    pub fn is_active(&self) -> bool {
        self.equipped
    }

    /// Generate the default AC formula string for the given armor type and base
    /// AC.
    pub fn default_ac_expr_str(armor_type: ArmorType, base_ac: u32) -> String {
        match armor_type {
            ArmorType::Light => format!("{base_ac} + DEX.MOD"),
            ArmorType::Medium => format!("{base_ac} + min(DEX.MOD, 2)"),
            ArmorType::Heavy => format!("{base_ac}"),
            ArmorType::Shield => format!("AC + {base_ac}"),
            ArmorType::Natural => String::new(),
        }
    }

    /// Generate and parse the default AC formula for the given armor type and
    /// base AC.
    pub fn default_ac_expr(armor_type: ArmorType, base_ac: u32) -> Option<Expr> {
        if armor_type == ArmorType::Natural {
            return None;
        }
        let s = Self::default_ac_expr_str(armor_type, base_ac);
        s.parse().ok()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WeaponEffect {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub damage_type: Option<DamageType>,
    #[serde(default)]
    pub expr: Expr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct Weapon {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_quantity")]
    pub quantity: u32,
    #[serde(default)]
    pub category: WeaponCategory,
    #[serde(default = "default_weapon_ability")]
    pub ability: Ability,
    #[serde(default)]
    pub magic_bonus: i32,
    #[serde(default)]
    pub attack_expr: Option<Expr>,
    #[serde(default)]
    pub effects: Vec<WeaponEffect>,
    #[serde(default)]
    pub equipped: bool,
    #[serde(default, skip_serializing_if = "Enchantment::is_empty")]
    pub magic: Enchantment,
}

fn default_quantity() -> u32 {
    1
}

fn default_weapon_ability() -> Ability {
    Ability::Strength
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            name: String::new(),
            quantity: default_quantity(),
            category: WeaponCategory::default(),
            ability: default_weapon_ability(),
            magic_bonus: 0,
            attack_expr: None,
            effects: Vec::new(),
            equipped: false,
            magic: Enchantment::default(),
        }
    }
}

impl Weapon {
    pub fn is_active(&self) -> bool {
        self.equipped
    }

    /// Generate the default attack-bonus formula for the given weapon
    /// parameters. The formula is fully reactive: it uses `PROF.XXX_WEAPONS`
    /// to include proficiency bonus only when the character is proficient,
    /// and `ATK` to incorporate global attack-bonus overrides from effects.
    pub fn default_attack_expr_str(
        category: WeaponCategory,
        ability: Ability,
        magic: i32,
    ) -> String {
        let ab = ability.abbr();
        let prof = match category {
            WeaponCategory::Simple => "PROF.SIMPLE_WEAPONS",
            WeaponCategory::Martial => "PROF.MARTIAL_WEAPONS",
        };
        let mut s = format!("{ab}.MOD + if({prof}, PROF.BONUS, 0) + ATK");
        if magic != 0 {
            s = format!("{s} {magic:+}");
        }
        s
    }

    pub fn default_attack_expr(
        category: WeaponCategory,
        ability: Ability,
        magic: i32,
    ) -> Option<Expr> {
        Self::default_attack_expr_str(category, ability, magic)
            .parse()
            .ok()
    }

    /// Returns the expression to evaluate for this weapon's attack bonus:
    /// the explicit `attack_expr` when set, otherwise the default derived
    /// from `category` / `ability` / `magic_bonus`.
    pub fn effective_attack_expr(&self) -> Expr {
        self.attack_expr.clone().unwrap_or_else(|| {
            Self::default_attack_expr(self.category, self.ability, self.magic_bonus)
                .unwrap_or_default()
        })
    }

    /// Default per-weapon damage formula: a single d4 plus the ability
    /// modifier. User can override the die in the UI.
    pub fn default_damage_expr_str(ability: Ability) -> String {
        format!("d4 + {}.MOD", ability.abbr())
    }

    pub fn default_damage_expr(ability: Ability) -> Expr {
        Self::default_damage_expr_str(ability)
            .parse()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Item {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub quantity: u32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub equipped: bool,
    #[serde(default, skip_serializing_if = "Enchantment::is_empty")]
    pub magic: Enchantment,
}

impl Item {
    pub fn is_active(&self) -> bool {
        self.equipped
    }
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

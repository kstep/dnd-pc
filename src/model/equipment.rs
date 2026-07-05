use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::{
    model::{Ability, ArmorType, Expr, ItemEffects, Money, WeaponCategory, Weight},
    rules::WhenCondition,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Equipment {
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub currency: Currency,
}

impl Equipment {
    /// `OnGearActive` only fires for equipped gear; other timings run on all
    /// gear.
    pub fn assignments(&self, when: WhenCondition) -> impl Iterator<Item = (usize, &Expr)> + '_ {
        let active_only = matches!(when, WhenCondition::OnGearActive);
        self.items
            .iter()
            .enumerate()
            .flat_map(move |(index, item)| {
                let gate = !active_only || item.is_active();
                item.effects
                    .assign
                    .iter()
                    .filter(move |assignment| gate && assignment.when == when)
                    .map(move |assignment| (index, &assignment.expr))
            })
    }

    /// Total carried weight: Σ quantity × per-unit weight.
    pub fn total_weight(&self) -> Weight {
        self.items.iter().map(Item::total_weight).sum()
    }
}

/// Kind-specific parameters. Pure data — every formula (attack bonus, AC)
/// is derived from these, never stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub enum ItemKind {
    #[default]
    Misc,
    Weapon {
        #[serde(default)]
        category: WeaponCategory,
        #[serde(default = "default_weapon_ability")]
        ability: Ability,
        #[serde(default)]
        magic_bonus: i32,
    },
    Armor {
        #[serde(default)]
        armor_type: ArmorType,
        #[serde(default)]
        base_ac: u32,
    },
}

impl ItemKind {
    pub fn default_weapon() -> Self {
        Self::Weapon {
            category: WeaponCategory::default(),
            ability: default_weapon_ability(),
            magic_bonus: 0,
        }
    }

    pub fn default_armor() -> Self {
        Self::Armor {
            armor_type: ArmorType::default(),
            base_ac: 11,
        }
    }

    /// Attack-bonus formula for the given weapon parameters. Reactive:
    /// proficiency gates through `PROF.*_WEAPONS`, global bonus via `ATK`.
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

    /// AC formula for the given armor type and base AC; None for Natural.
    pub fn default_ac_expr_str(armor_type: ArmorType, base_ac: u32) -> Option<String> {
        match armor_type {
            ArmorType::Light => Some(format!("{base_ac} + DEX.MOD")),
            ArmorType::Medium => Some(format!("{base_ac} + min(DEX.MOD, 2)")),
            ArmorType::Heavy => Some(format!("{base_ac}")),
            ArmorType::Shield => Some(format!("AC + {base_ac}")),
            ArmorType::Natural => None,
        }
    }

    /// Default per-weapon damage formula: a single d4 plus the ability
    /// modifier.
    pub fn default_damage_expr_str(ability: Ability) -> String {
        format!("d4 + {}.MOD", ability.abbr())
    }
}

fn default_quantity() -> u32 {
    1
}

fn default_weapon_ability() -> Ability {
    Ability::Strength
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct Item {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_quantity")]
    pub quantity: u32,
    #[serde(default)]
    pub weight: Weight,
    #[serde(default)]
    pub price: Money,
    #[serde(default)]
    pub equipped: bool,
    #[serde(default)]
    pub kind: ItemKind,
    #[serde(default, skip_serializing_if = "ItemEffects::is_empty")]
    pub effects: ItemEffects,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            quantity: default_quantity(),
            weight: Weight::default(),
            price: Money::default(),
            equipped: false,
            kind: ItemKind::default(),
            effects: ItemEffects::default(),
        }
    }
}

impl Item {
    pub fn is_active(&self) -> bool {
        self.equipped
    }

    pub fn is_weapon(&self) -> bool {
        matches!(self.kind, ItemKind::Weapon { .. })
    }

    pub fn is_armor(&self) -> bool {
        matches!(self.kind, ItemKind::Armor { .. })
    }

    /// Derived attack-bonus expression; None for non-weapons.
    pub fn attack_expr(&self) -> Option<Expr> {
        let ItemKind::Weapon {
            category,
            ability,
            magic_bonus,
        } = self.kind
        else {
            return None;
        };
        ItemKind::default_attack_expr_str(category, ability, magic_bonus)
            .parse()
            .ok()
    }

    /// Derived AC expression; None for non-armor and Natural armor.
    pub fn ac_expr(&self) -> Option<Expr> {
        let ItemKind::Armor {
            armor_type,
            base_ac,
        } = self.kind
        else {
            return None;
        };
        ItemKind::default_ac_expr_str(armor_type, base_ac)?
            .parse()
            .ok()
    }

    pub fn total_weight(&self) -> Weight {
        self.weight * self.quantity
    }

    /// Spend an activation: `cost` charges (clamped to max) and `consumes`
    /// units of quantity (floored at 0).
    pub fn activate(&mut self, cost: u32, consumes: u32) {
        if let Some(ref mut charges) = self.effects.charges {
            charges.used = (charges.used + cost).min(charges.max);
        }
        self.quantity = self.quantity.saturating_sub(consumes);
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

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::{model::Charges, rules::feature::Assignment};

    fn weapon(name: &str, category: WeaponCategory, ability: Ability, magic: i32) -> Item {
        Item {
            name: name.into(),
            kind: ItemKind::Weapon {
                category,
                ability,
                magic_bonus: magic,
            },
            ..Item::default()
        }
    }

    #[wasm_bindgen_test]
    fn attack_expr_derives_from_parameters() {
        let simple = weapon("Club", WeaponCategory::Simple, Ability::Strength, 0);
        assert_eq!(
            simple.attack_expr().unwrap().to_string(),
            "STR.MOD + if(PROF.SIMPLE_WEAPONS, PROF.BONUS, 0) + ATK"
        );
        let martial = weapon("Rapier", WeaponCategory::Martial, Ability::Dexterity, 1);
        assert_eq!(
            martial.attack_expr().unwrap().to_string(),
            "DEX.MOD + if(PROF.MARTIAL_WEAPONS, PROF.BONUS, 0) + ATK + 1"
        );
        let cursed = weapon("Cursed", WeaponCategory::Martial, Ability::Strength, -2);
        assert_eq!(
            cursed.attack_expr().unwrap().to_string(),
            "STR.MOD + if(PROF.MARTIAL_WEAPONS, PROF.BONUS, 0) + ATK - 2"
        );
    }

    #[wasm_bindgen_test]
    fn attack_expr_none_for_non_weapons() {
        assert!(Item::default().attack_expr().is_none());
        let armor = Item {
            kind: ItemKind::default_armor(),
            ..Item::default()
        };
        assert!(armor.attack_expr().is_none());
    }

    #[wasm_bindgen_test]
    fn ac_expr_derives_from_parameters() {
        let cases = [
            (ArmorType::Light, 11, Some("11 + DEX.MOD")),
            (ArmorType::Medium, 14, Some("14 + min(DEX.MOD, 2)")),
            (ArmorType::Heavy, 18, Some("18")),
            (ArmorType::Shield, 2, Some("AC + 2")),
            (ArmorType::Natural, 13, None),
        ];
        for (armor_type, base_ac, expected) in cases {
            let armor = Item {
                kind: ItemKind::Armor {
                    armor_type,
                    base_ac,
                },
                ..Item::default()
            };
            assert_eq!(
                armor.ac_expr().map(|expr| expr.to_string()),
                expected.map(str::to_string),
                "{armor_type:?}"
            );
        }
        assert!(Item::default().ac_expr().is_none());
    }

    #[wasm_bindgen_test]
    fn assignments_gate_on_gear_active_only() {
        let mut equipment = Equipment::default();
        for (equipped, kind) in [
            (false, ItemKind::default_weapon()),
            (true, ItemKind::default_armor()),
            (false, ItemKind::Misc),
        ] {
            let mut item = Item {
                equipped,
                kind,
                ..Item::default()
            };
            item.effects.assign.push(Assignment {
                expr: "AC += 1".parse().unwrap(),
                when: WhenCondition::OnGearActive,
            });
            item.effects.assign.push(Assignment {
                expr: "CHARGES.USED = 0".parse().unwrap(),
                when: WhenCondition::OnLongRest,
            });
            equipment.items.push(item);
        }

        // OnGearActive: only the equipped armor row fires.
        let active: Vec<usize> = equipment
            .assignments(WhenCondition::OnGearActive)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(active, vec![1]);

        // OnLongRest fires for everything regardless of equipped.
        let rest: Vec<usize> = equipment
            .assignments(WhenCondition::OnLongRest)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(rest, vec![0, 1, 2]);
    }

    #[wasm_bindgen_test]
    fn activate_spends_charges_and_quantity() {
        let mut item = Item {
            quantity: 2,
            ..Item::default()
        };
        item.effects.charges = Some(Charges { used: 6, max: 7 });
        item.activate(3, 1);
        assert_eq!(item.effects.charges.unwrap().used, 7, "clamped to max");
        assert_eq!(item.quantity, 1);
        item.activate(0, 5);
        assert_eq!(item.quantity, 0, "floored at zero");
    }

    #[wasm_bindgen_test]
    fn total_weight_sums_quantities() {
        let mut equipment = Equipment::default();
        equipment.items.push(Item {
            quantity: 4,
            weight: Weight(150),
            ..Item::default()
        });
        equipment.items.push(Item {
            quantity: 1,
            weight: Weight(25),
            ..Item::default()
        });
        assert_eq!(equipment.total_weight(), Weight(625));
    }
}

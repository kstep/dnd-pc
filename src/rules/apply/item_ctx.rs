use crate::{
    expr,
    model::{Attribute, Character, Charges, Enchantment, Expr, GearRef},
    rules::WhenCondition,
};

/// Apply-time context for evaluating gear `Enchantment.assign` expressions.
///
/// Routes the gear-local attributes (`Charges`, `ChargesMax`, `ChargesUsed`,
/// `Quantity`) to the owning gear's `magic.charges` / `quantity`. Anything
/// else falls through to `Character::assign` / `Character::resolve` so
/// expressions can still read character state (`HP`, `LEVEL`, …).
pub struct ItemApplyCtx<'a> {
    pub character: &'a mut Character,
    pub gear: GearRef,
}

impl ItemApplyCtx<'_> {
    fn enchantment(&self) -> Option<&Enchantment> {
        match self.gear {
            GearRef::Item(i) => self.character.equipment.items.get(i).map(|x| &x.magic),
            GearRef::Weapon(i) => self.character.equipment.weapons.get(i).map(|x| &x.magic),
            GearRef::Armor(i) => self.character.equipment.armors.get(i).map(|x| &x.magic),
        }
    }

    fn enchantment_mut(&mut self) -> Option<&mut Enchantment> {
        match self.gear {
            GearRef::Item(i) => self
                .character
                .equipment
                .items
                .get_mut(i)
                .map(|x| &mut x.magic),
            GearRef::Weapon(i) => self
                .character
                .equipment
                .weapons
                .get_mut(i)
                .map(|x| &mut x.magic),
            GearRef::Armor(i) => self
                .character
                .equipment
                .armors
                .get_mut(i)
                .map(|x| &mut x.magic),
        }
    }

    fn quantity(&self) -> u32 {
        match self.gear {
            GearRef::Item(i) => self
                .character
                .equipment
                .items
                .get(i)
                .map(|x| x.quantity)
                .unwrap_or(0),
            GearRef::Weapon(i) => self
                .character
                .equipment
                .weapons
                .get(i)
                .map(|x| x.quantity)
                .unwrap_or(0),
            GearRef::Armor(_) => 1,
        }
    }

    fn charges_field<R>(&self, read: impl Fn(&Charges) -> R, default: R) -> R {
        self.enchantment()
            .and_then(|e| e.charges.as_ref())
            .map(read)
            .unwrap_or(default)
    }

    fn charges_mut_or_init(&mut self) -> Option<&mut Charges> {
        let ench = self.enchantment_mut()?;
        Some(ench.charges.get_or_insert_with(Charges::default))
    }
}

impl expr::Context<Attribute, i32> for ItemApplyCtx<'_> {
    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        match var {
            Attribute::Charges => {
                let Some(charges) = self.charges_mut_or_init() else {
                    return Ok(());
                };
                let max_i = charges.max as i32;
                charges.used = (max_i - value).clamp(0, max_i) as u32;
                Ok(())
            }
            Attribute::ChargesMax => {
                let Some(charges) = self.charges_mut_or_init() else {
                    return Ok(());
                };
                charges.max = value.max(0) as u32;
                if charges.used > charges.max {
                    charges.used = charges.max;
                }
                Ok(())
            }
            Attribute::ChargesUsed => {
                let Some(charges) = self.charges_mut_or_init() else {
                    return Ok(());
                };
                let max_i = charges.max as i32;
                charges.used = value.clamp(0, max_i) as u32;
                Ok(())
            }
            Attribute::Quantity => {
                log::warn!("Attribute::Quantity is read-only via ItemApplyCtx");
                Ok(())
            }
            _ => self.character.assign(var, value),
        }
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::Charges => Ok(self.charges_field(|c| c.available(), 0) as i32),
            Attribute::ChargesMax => Ok(self.charges_field(|c| c.max, 0) as i32),
            Attribute::ChargesUsed => Ok(self.charges_field(|c| c.used, 0) as i32),
            Attribute::Quantity => Ok(self.quantity() as i32),
            _ => self.character.resolve(var),
        }
    }
}

pub fn assign_items(character: &mut Character, when: WhenCondition) {
    let work: Vec<(GearRef, Expr)> = character
        .equipment
        .assignments(when)
        .map(|(gear, expr)| (gear, expr.clone()))
        .collect();
    for (gear, expr) in work {
        let mut ctx = ItemApplyCtx { character, gear };
        if let Err(error) = expr.apply(&mut ctx) {
            log::debug!("Gear assignment failed for {gear:?}: {error:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::{
        model::{Charges, Item},
        rules::feature::Assignment,
    };

    fn item_with_assign(name: &str, max: u32, used: u32, expr: &str, when: WhenCondition) -> Item {
        let mut item = Item {
            name: name.to_string(),
            quantity: 1,
            ..Default::default()
        };
        item.magic.charges = Some(Charges { used, max });
        item.magic.assign.push(Assignment {
            expr: expr.parse().unwrap(),
            when,
        });
        item
    }

    #[wasm_bindgen_test]
    fn long_rest_refills_charges_to_zero_used() {
        let mut character = Character::new();
        character.equipment.items.push(item_with_assign(
            "Wand",
            7,
            5,
            "CHARGES.USED = 0",
            WhenCondition::OnLongRest,
        ));

        assign_items(&mut character, WhenCondition::OnLongRest);

        let item = &character.equipment.items[0];
        assert_eq!(item.magic.charges.as_ref().unwrap().used, 0);
        assert_eq!(item.magic.charges.as_ref().unwrap().max, 7);
    }

    #[wasm_bindgen_test]
    fn short_rest_partial_refill() {
        let mut character = Character::new();
        character.equipment.items.push(item_with_assign(
            "Wand",
            10,
            8,
            // dice-less for deterministic test; semantics still: subtract 3 from used
            "CHARGES.USED -= 3",
            WhenCondition::OnShortRest,
        ));

        assign_items(&mut character, WhenCondition::OnShortRest);

        assert_eq!(
            character.equipment.items[0]
                .magic
                .charges
                .as_ref()
                .unwrap()
                .used,
            5
        );
    }

    #[wasm_bindgen_test]
    fn long_rest_fires_regardless_of_equipped() {
        let mut character = Character::new();
        let mut item =
            item_with_assign("Wand", 7, 5, "CHARGES.USED = 0", WhenCondition::OnLongRest);
        item.equipped = false; // explicitly NOT equipped
        character.equipment.items.push(item);

        assign_items(&mut character, WhenCondition::OnLongRest);

        // refill happens anyway: gear lives its own life
        assert_eq!(
            character.equipment.items[0]
                .magic
                .charges
                .as_ref()
                .unwrap()
                .used,
            0
        );
    }

    #[wasm_bindgen_test]
    fn quantity_writes_are_no_op() {
        let mut character = Character::new();
        let mut item = Item {
            name: "Bag".to_string(),
            quantity: 3,
            ..Default::default()
        };
        item.magic.assign.push(Assignment {
            expr: "QUANTITY = 0".parse().unwrap(),
            when: WhenCondition::OnLongRest,
        });
        character.equipment.items.push(item);

        assign_items(&mut character, WhenCondition::OnLongRest);

        // Quantity is read-only via ItemApplyCtx; structural decrement
        // happens through ChoiceOption.consumes only.
        assert_eq!(character.equipment.items[0].quantity, 3);
    }
}

use crate::{
    expr,
    model::{Attribute, Character, CharacterCore, Charges, Expr, ItemEffects},
    rules::WhenCondition,
};

/// Apply-time context for evaluating gear `ItemEffects.assign` expressions.
///
/// Routes the gear-local attributes (`Charges`, `ChargesMax`, `ChargesUsed`,
/// `Quantity`) to the owning item's `effects.charges` / `quantity`. Anything
/// else falls through to `Character::assign` / `Character::resolve` so
/// expressions can still read character state (`HP`, `LEVEL`, …).
pub struct ItemApplyCtx<'a> {
    pub character: &'a mut Character,
    pub gear: usize,
}

impl ItemApplyCtx<'_> {
    fn item_effects(&self) -> Option<&ItemEffects> {
        self.character
            .equipment
            .items
            .get(self.gear)
            .map(|item| &item.effects)
    }

    fn item_effects_mut(&mut self) -> Option<&mut ItemEffects> {
        self.character
            .equipment
            .items
            .get_mut(self.gear)
            .map(|item| &mut item.effects)
    }

    fn quantity(&self) -> u32 {
        self.character
            .equipment
            .items
            .get(self.gear)
            .map(|item| item.quantity)
            .unwrap_or(0)
    }

    fn charges_field<R>(&self, read: impl Fn(&Charges) -> R, default: R) -> R {
        self.item_effects()
            .and_then(|effects| effects.charges.as_ref())
            .map(read)
            .unwrap_or(default)
    }

    fn charges_mut_or_init(&mut self) -> Option<&mut Charges> {
        let effects = self.item_effects_mut()?;
        Some(effects.charges.get_or_insert_with(Charges::default))
    }
}

impl AsRef<CharacterCore> for ItemApplyCtx<'_> {
    fn as_ref(&self) -> &CharacterCore {
        &self.character.core
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
    let work: Vec<(usize, Expr)> = character
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
        item.effects.charges = Some(Charges { used, max });
        item.effects.assign.push(Assignment {
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
        assert_eq!(item.effects.charges.as_ref().unwrap().used, 0);
        assert_eq!(item.effects.charges.as_ref().unwrap().max, 7);
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
                .effects
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
                .effects
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
            name: "Bag".into(),
            quantity: 3,
            ..Default::default()
        };
        item.effects.assign.push(Assignment {
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

use crate::{
    expr,
    model::{
        Ability, AssignInputs, AttrKey, Attribute, CharacterCore, ClassLevel, Die, Feature,
        FeatureField, FeatureOption, FeatureSource, FeatureValue, Spell, SpellData, SpellSlotPool,
    },
    rules::{
        WhenCondition,
        apply::{IdentityChange, args_ctx::WithArgs},
        feature::Assignment,
    },
};

/// Apply-time context for evaluating feature assignment expressions.
///
/// Holds a mutable reference to the character plus an index into
/// `character.features.list` identifying the feature whose assigns are
/// running. `expr_index` identifies which interactive assignment is
/// currently evaluated, so `Arg(n)` resolves to
/// `feature.inputs[expr_index].args[n]` directly inside `resolve()`.
pub struct ApplyContext<'a> {
    pub character: &'a mut CharacterCore,
    pub feature_pos: usize,
    pub expr_index: usize,
    /// Identity mutations observed during this context's `run_assignments`
    /// pass. Drained by the caller via [`Self::take_identity_changes`].
    pub identity_changes: Vec<IdentityChange>,
}

impl<'a> ApplyContext<'a> {
    pub fn new(character: &'a mut CharacterCore, feature_pos: usize) -> Self {
        Self {
            character,
            feature_pos,
            expr_index: 0,
            identity_changes: Vec::new(),
        }
    }

    /// Drain the recorded identity changes. Outer apply layers process them
    /// to update `applied` flags and derive feature follow-ups.
    pub fn take_identity_changes(&mut self) -> Vec<IdentityChange> {
        std::mem::take(&mut self.identity_changes)
    }

    pub fn current_feature(&self) -> &Feature {
        self.character
            .features
            .at(self.feature_pos)
            .expect("feature_pos out of bounds")
    }

    pub fn current_feature_name(&self) -> &str {
        &self.current_feature().name
    }

    /// Current level of the class that owns the scoped feature, looked up
    /// via `identity.classes`. Returns 0 for non-Class sources (Species,
    /// Background, User). The level inside the `FeatureSource::Class(_, N)`
    /// variant is the level at which the feature was *granted*, not the
    /// current level of the class — assigns that scale with progression
    /// (e.g. `POINTS.MAX = CLASS.LEVEL`) need the latter.
    pub fn class_level(&self) -> i32 {
        self.scoped_class().map(|cl| cl.level as i32).unwrap_or(0)
    }

    /// Class entry that owns the scoped feature, or None for non-Class
    /// sources (Species, Background, User). Same scope semantics as
    /// `class_level()` but exposes the full `ClassLevel` for callers that
    /// need hit-die fields too.
    fn scoped_class(&self) -> Option<&ClassLevel> {
        let class_name: &str = match &self.current_feature().source {
            FeatureSource::Class(name, _) | FeatureSource::Subclass(name, _, _) => name,
            _ => return None,
        };
        self.character
            .identity
            .classes
            .iter()
            .find(|class_level| &*class_level.class == class_name)
    }

    fn scoped_class_mut(&mut self) -> Option<&mut ClassLevel> {
        // Split the &mut CharacterCore into disjoint &features + &mut identity
        // so class name can be read from one half and classes mutated through
        // the other without an intermediate String allocation.
        let core = &mut *self.character;
        let scoped_feature = core
            .features
            .at(self.feature_pos)
            .expect("feature_pos out of bounds");
        let class_name: &str = match &scoped_feature.source {
            FeatureSource::Class(name, _) | FeatureSource::Subclass(name, _, _) => name,
            _ => return None,
        };
        core.identity
            .classes
            .iter_mut()
            .find(|class_level| &*class_level.class == class_name)
    }

    /// Iterate `assignments`, evaluating each whose `when` matches. For
    /// interactive assignments, `Arg(n)` resolves through `expr_index`
    /// pointing at the matching `feature.inputs` slot — read inside
    /// `resolve()`. Failures log at debug level: production paths rarely
    /// surface them, the args-modal cascade legitimately fails while the
    /// user is mid-typing.
    pub fn run_assignments(&mut self, assignments: &[Assignment], when: WhenCondition) {
        self.expr_index = 0;
        for assign in assignments.iter().filter(|assign| assign.when == when) {
            let interactive = assign.is_interactive();
            let dice = interactive
                .then(|| self.current_feature().inputs.get(self.expr_index))
                .flatten()
                .map(|input| input.dice.clone())
                .filter(|dice| !dice.is_empty());
            let result = if let Some(dice) = dice {
                assign.expr.apply_with_dice(self, &dice)
            } else {
                assign.expr.apply(self)
            };
            if let Err(error) = result {
                let feature_name = self.current_feature_name();
                log::debug!("Apply assignment failed for '{feature_name}': {error:?}");
            }
            if interactive {
                self.expr_index += 1;
            }
        }
    }

    /// Find a `FeatureField` by name across all applied features. Search
    /// order matches `features.list`, not BTreeMap key order. Returns
    /// `(feature_pos, field_index)` on first match.
    fn find_field_named(&self, field_name: &str) -> Option<(usize, usize)> {
        for (feat_idx, feature) in self.character.features.iter().enumerate() {
            let Some(feature_data) = self.character.features.get(&*feature.name) else {
                continue;
            };
            for (field_idx, field) in feature_data.fields.iter().enumerate() {
                if field.name == field_name {
                    return Some((feat_idx, field_idx));
                }
            }
        }
        None
    }

    /// Append a default-shaped `FeatureField` to `feature_name`'s data.
    /// Returns the new field's index.
    fn lazy_create_field(
        &mut self,
        feature_name: &str,
        field_name: &str,
        initial_value: FeatureValue,
    ) -> usize {
        let feature_data = self
            .character
            .features
            .entry(Box::from(feature_name))
            .or_default();
        feature_data.fields.push(FeatureField {
            name: field_name.to_string(),
            label: None,
            description: String::new(),
            value: initial_value,
        });
        feature_data.fields.len() - 1
    }

    fn named_field_value(&self, name: &str) -> Option<&FeatureValue> {
        let (feat_idx, field_idx) = self.find_field_named(name)?;
        let feature_name = self.character.features.at(feat_idx)?.name.as_ref();
        let feature_data = self.character.features.get(feature_name)?;
        feature_data.fields.get(field_idx).map(|field| &field.value)
    }

    fn named_field_value_mut(&mut self, name: &str) -> Option<&mut FeatureValue> {
        let (feat_idx, field_idx) = self.find_field_named(name)?;
        let (list, data) = self.character.features.split_mut();
        let feature_name = &*list[feat_idx].name;
        let feature_data = data.get_mut(feature_name)?;
        feature_data
            .fields
            .get_mut(field_idx)
            .map(|field| &mut field.value)
    }

    /// First Points/Die field of the current scoped feature (or None if the
    /// feature has neither). Used by Scoped `POINTS`/`POINTS.MAX` lookups.
    fn scoped_pool_field(&self) -> Option<&FeatureValue> {
        let feature_data = self.character.features.get(self.current_feature_name())?;
        feature_data
            .fields
            .iter()
            .map(|field| &field.value)
            .find(|value| {
                matches!(
                    value,
                    FeatureValue::Points { .. } | FeatureValue::Die { .. }
                )
            })
    }

    fn scoped_pool_field_mut(&mut self) -> Option<&mut FeatureValue> {
        let (list, data) = self.character.features.split_mut();
        let feature_name = &*list[self.feature_pos].name;
        let feature_data = data.get_mut(feature_name)?;
        feature_data
            .fields
            .iter_mut()
            .map(|field| &mut field.value)
            .find(|value| {
                matches!(
                    value,
                    FeatureValue::Points { .. } | FeatureValue::Die { .. }
                )
            })
    }

    /// Resolve a `Named`-key field via `extract`. Errors on `Scoped`, on
    /// missing field, or on type mismatch (where `extract` returns None).
    fn resolve_named_field(
        &self,
        key: AttrKey,
        error_var: impl Fn() -> Attribute,
        extract: impl FnOnce(&FeatureValue) -> Option<i32>,
    ) -> Result<i32, expr::Error> {
        let AttrKey::Named(name) = key else {
            return Err(expr::Error::unsupported_var(error_var()));
        };
        self.named_field_value(name)
            .and_then(extract)
            .ok_or_else(|| expr::Error::unsupported_var(error_var()))
    }

    /// Mutate a `Named`-key field via `update`, or lazy-create with
    /// `default` in the scoped feature if missing. `update` returns `Err`
    /// on type mismatch with the existing field.
    fn assign_named_field(
        &mut self,
        key: AttrKey,
        error_var: impl Fn() -> Attribute,
        default: FeatureValue,
        update: impl FnOnce(&mut FeatureValue) -> Result<(), ()>,
    ) -> Result<(), expr::Error> {
        let AttrKey::Named(name) = key else {
            return Err(expr::Error::unsupported_var(error_var()));
        };
        if let Some(field_value) = self.named_field_value_mut(name) {
            return update(field_value).map_err(|_| expr::Error::unsupported_var(error_var()));
        }
        let scope = self.current_feature_name().to_string();
        self.lazy_create_field(&scope, name, default);
        Ok(())
    }

    fn resolve_points(&self, key: AttrKey) -> Result<i32, expr::Error> {
        match key {
            AttrKey::Scoped => match self.scoped_pool_field() {
                Some(FeatureValue::Points { used, max }) => Ok((*max as i32 - *used as i32).max(0)),
                Some(FeatureValue::Die { die, used }) => {
                    Ok((die.amount as i32 - *used as i32).max(0))
                }
                _ => Err(expr::Error::unsupported_var(Attribute::Points(key))),
            },
            AttrKey::Named(_) => self.resolve_named_field(
                key,
                || Attribute::Points(key),
                |value| match value {
                    FeatureValue::Points { used, max } => Some((*max as i32 - *used as i32).max(0)),
                    _ => None,
                },
            ),
        }
    }

    fn resolve_points_max(&self, key: AttrKey) -> Result<i32, expr::Error> {
        match key {
            AttrKey::Scoped => match self.scoped_pool_field() {
                Some(FeatureValue::Points { max, .. }) => Ok(*max as i32),
                Some(FeatureValue::Die { die, .. }) => Ok(die.amount as i32),
                _ => Err(expr::Error::unsupported_var(Attribute::PointsMax(key))),
            },
            AttrKey::Named(_) => self.resolve_named_field(
                key,
                || Attribute::PointsMax(key),
                |value| match value {
                    FeatureValue::Points { max, .. } => Some(*max as i32),
                    _ => None,
                },
            ),
        }
    }

    fn resolve_die_sides(&self, key: AttrKey) -> Result<i32, expr::Error> {
        self.resolve_named_field(
            key,
            || Attribute::DieSides(key),
            |value| match value {
                FeatureValue::Die { die, .. } => Some(die.sides as i32),
                _ => None,
            },
        )
    }

    fn resolve_die_count(&self, key: AttrKey) -> Result<i32, expr::Error> {
        self.resolve_named_field(
            key,
            || Attribute::DieCount(key),
            |value| match value {
                FeatureValue::Die { die, .. } => Some(die.amount as i32),
                _ => None,
            },
        )
    }

    fn resolve_die_used(&self, key: AttrKey) -> Result<i32, expr::Error> {
        self.resolve_named_field(
            key,
            || Attribute::DieUsed(key),
            |value| match value {
                FeatureValue::Die { used, .. } => Some(*used as i32),
                _ => None,
            },
        )
    }

    fn resolve_bonus(&self, key: AttrKey) -> Result<i32, expr::Error> {
        self.resolve_named_field(
            key,
            || Attribute::Bonus(key),
            |value| match value {
                FeatureValue::Bonus(stored) => Some(*stored),
                _ => None,
            },
        )
    }

    fn resolve_choice_count(&self, key: AttrKey) -> Result<i32, expr::Error> {
        self.resolve_named_field(
            key,
            || Attribute::ChoiceCount(key),
            |value| match value {
                FeatureValue::Choice { options } => Some(options.len() as i32),
                _ => None,
            },
        )
    }

    fn assign_points(&mut self, key: AttrKey, value: i32) -> Result<(), expr::Error> {
        match key {
            AttrKey::Scoped => match self.scoped_pool_field_mut() {
                Some(FeatureValue::Points { used, max }) => {
                    let max_i = *max as i32;
                    *used = (max_i - value).clamp(0, max_i) as u32;
                    Ok(())
                }
                Some(FeatureValue::Die { die, used }) => {
                    let amount_i = die.amount as i32;
                    *used = (amount_i - value).clamp(0, amount_i) as u32;
                    Ok(())
                }
                _ => Err(expr::Error::unsupported_var(Attribute::Points(key))),
            },
            AttrKey::Named(_) => {
                let max = value.max(0) as u32;
                self.assign_named_field(
                    key,
                    || Attribute::Points(key),
                    FeatureValue::Points { used: 0, max },
                    |field_value| match field_value {
                        FeatureValue::Points { used, max } => {
                            let max_i = *max as i32;
                            *used = (max_i - value).clamp(0, max_i) as u32;
                            Ok(())
                        }
                        _ => Err(()),
                    },
                )
            }
        }
    }

    fn assign_points_max(&mut self, key: AttrKey, value: i32) -> Result<(), expr::Error> {
        let new_max = value.max(0) as u32;
        match key {
            AttrKey::Scoped => match self.scoped_pool_field_mut() {
                Some(FeatureValue::Points { max, used }) => {
                    *max = new_max;
                    if *used > new_max {
                        *used = new_max;
                    }
                    Ok(())
                }
                Some(FeatureValue::Die { die, used }) => {
                    die.amount = new_max;
                    if *used > new_max {
                        *used = new_max;
                    }
                    Ok(())
                }
                _ => Err(expr::Error::unsupported_var(Attribute::PointsMax(key))),
            },
            AttrKey::Named(_) => self.assign_named_field(
                key,
                || Attribute::PointsMax(key),
                FeatureValue::Points {
                    used: 0,
                    max: new_max,
                },
                |field_value| match field_value {
                    FeatureValue::Points { max, used } => {
                        *max = new_max;
                        if *used > new_max {
                            *used = new_max;
                        }
                        Ok(())
                    }
                    _ => Err(()),
                },
            ),
        }
    }

    fn assign_die_sides(&mut self, key: AttrKey, value: i32) -> Result<(), expr::Error> {
        let new_sides = value.max(0) as u32;
        self.assign_named_field(
            key,
            || Attribute::DieSides(key),
            FeatureValue::Die {
                die: Die {
                    sides: new_sides,
                    amount: 0,
                },
                used: 0,
            },
            |field_value| match field_value {
                FeatureValue::Die { die, .. } => {
                    die.sides = new_sides;
                    Ok(())
                }
                _ => Err(()),
            },
        )
    }

    fn assign_die_count(&mut self, key: AttrKey, value: i32) -> Result<(), expr::Error> {
        let new_amount = value.max(0) as u32;
        self.assign_named_field(
            key,
            || Attribute::DieCount(key),
            FeatureValue::Die {
                die: Die {
                    sides: 0,
                    amount: new_amount,
                },
                used: 0,
            },
            |field_value| match field_value {
                FeatureValue::Die { die, used } => {
                    die.amount = new_amount;
                    if *used > new_amount {
                        *used = new_amount;
                    }
                    Ok(())
                }
                _ => Err(()),
            },
        )
    }

    fn assign_die_used(&mut self, key: AttrKey, value: i32) -> Result<(), expr::Error> {
        self.assign_named_field(
            key,
            || Attribute::DieUsed(key),
            FeatureValue::Die {
                die: Die::default(),
                used: 0,
            },
            |field_value| match field_value {
                FeatureValue::Die { die, used } => {
                    *used = value.clamp(0, die.amount as i32) as u32;
                    Ok(())
                }
                _ => Err(()),
            },
        )
    }

    fn assign_bonus(&mut self, key: AttrKey, value: i32) -> Result<(), expr::Error> {
        self.assign_named_field(
            key,
            || Attribute::Bonus(key),
            FeatureValue::Bonus(value),
            |field_value| match field_value {
                FeatureValue::Bonus(stored) => {
                    *stored = value;
                    Ok(())
                }
                _ => Err(()),
            },
        )
    }

    fn assign_choice_count(&mut self, key: AttrKey, value: i32) -> Result<(), expr::Error> {
        let count = value.max(0) as usize;
        self.assign_named_field(
            key,
            || Attribute::ChoiceCount(key),
            FeatureValue::Choice {
                options: vec![FeatureOption::default(); count],
            },
            |field_value| match field_value {
                FeatureValue::Choice { options } => {
                    options.resize_with(count, FeatureOption::default);
                    Ok(())
                }
                _ => Err(()),
            },
        )
    }

    fn feature_spell_data(&self) -> Option<&SpellData> {
        let name = self.current_feature_name();
        self.character.features.spell_data(name)
    }

    fn feature_spell_data_mut(&mut self) -> Option<&mut SpellData> {
        let (list, data) = self.character.features.split_mut();
        let name = &*list[self.feature_pos].name;
        data.get_mut(name).and_then(|entry| entry.spells.as_mut())
    }

    /// Lazy-create the scoped feature's `SpellData` skeleton.
    fn feature_spell_data_or_create(&mut self) -> &mut SpellData {
        let (list, data) = self.character.features.split_mut();
        let name = &*list[self.feature_pos].name;
        // BTreeMap::entry needs an owned key; the alloc is unavoidable when
        // creating a fresh data row for a feature that hasn't been touched
        // yet. The lookup-only path above stays zero-alloc.
        data.entry(Box::from(name))
            .or_default()
            .spells
            .get_or_insert_with(SpellData::default)
    }

    fn resolve_sticky(&self, key: AttrKey) -> Result<i32, expr::Error> {
        let AttrKey::Named(spell_name) = key else {
            return Err(expr::Error::unsupported_var(Attribute::Sticky(key)));
        };
        let Some(spell_data) = self.feature_spell_data() else {
            return Ok(0);
        };
        let exists = spell_data
            .spells
            .iter()
            .any(|spell| spell.sticky && spell.name == spell_name);
        Ok(if exists { 1 } else { 0 })
    }

    fn assign_sticky(&mut self, key: AttrKey, value: i32) -> Result<(), expr::Error> {
        let AttrKey::Named(spell_name) = key else {
            return Err(expr::Error::unsupported_var(Attribute::Sticky(key)));
        };
        let spell_name = spell_name.to_string();
        if value == 0 {
            let Some(spell_data) = self.feature_spell_data_mut() else {
                return Ok(());
            };
            spell_data
                .spells
                .retain(|spell| !(spell.sticky && spell.name == spell_name));
            return Ok(());
        }
        let spell_data = self.feature_spell_data_or_create();
        let already_present = spell_data
            .spells
            .iter()
            .any(|spell| spell.sticky && spell.name == spell_name);
        if already_present {
            return Ok(());
        }
        spell_data.spells.push(Spell {
            name: spell_name,
            sticky: true,
            ..Default::default()
        });
        Ok(())
    }

    fn resolve_free_uses(&self, key: AttrKey) -> Result<i32, expr::Error> {
        let AttrKey::Named(spell_name) = key else {
            return Err(expr::Error::unsupported_var(Attribute::FreeUses(key)));
        };
        let spell_data = self
            .feature_spell_data()
            .ok_or_else(|| expr::Error::unsupported_var(Attribute::FreeUses(key)))?;
        let spell = spell_data
            .spells
            .iter()
            .find(|spell| spell.name == spell_name)
            .ok_or_else(|| expr::Error::unsupported_var(Attribute::FreeUses(key)))?;
        Ok(spell
            .free_uses
            .as_ref()
            .map(|fu| fu.max as i32)
            .unwrap_or(0))
    }

    fn assign_free_uses(&mut self, key: AttrKey, value: i32) -> Result<(), expr::Error> {
        let new_max = value.max(0) as u32;
        match key {
            AttrKey::Named(spell_name) => {
                let spell_name = spell_name.to_string();
                let spell_data = self.feature_spell_data_mut().ok_or_else(|| {
                    expr::Error::unsupported_var(Attribute::FreeUses(AttrKey::named(&spell_name)))
                })?;
                let spell = spell_data
                    .spells
                    .iter_mut()
                    .find(|spell| spell.name == spell_name)
                    .ok_or_else(|| {
                        expr::Error::unsupported_var(Attribute::FreeUses(AttrKey::named(
                            &spell_name,
                        )))
                    })?;
                spell.set_free_uses_max(new_max);
                Ok(())
            }
            AttrKey::Scoped => {
                let Some(spell_data) = self.feature_spell_data_mut() else {
                    return Ok(());
                };
                for spell in spell_data.spells.iter_mut().filter(|spell| spell.cost > 0) {
                    spell.set_free_uses_max(new_max);
                }
                Ok(())
            }
        }
    }

    fn resolve_free_uses_used(&self, key: AttrKey) -> Result<i32, expr::Error> {
        let AttrKey::Named(spell_name) = key else {
            return Err(expr::Error::unsupported_var(Attribute::FreeUsesUsed(key)));
        };
        let spell_data = self
            .feature_spell_data()
            .ok_or_else(|| expr::Error::unsupported_var(Attribute::FreeUsesUsed(key)))?;
        let spell = spell_data
            .spells
            .iter()
            .find(|spell| spell.name == spell_name)
            .ok_or_else(|| expr::Error::unsupported_var(Attribute::FreeUsesUsed(key)))?;
        Ok(spell
            .free_uses
            .as_ref()
            .map(|fu| fu.used as i32)
            .unwrap_or(0))
    }

    fn assign_free_uses_used(&mut self, key: AttrKey, value: i32) -> Result<(), expr::Error> {
        match key {
            AttrKey::Named(spell_name) => {
                let spell_name = spell_name.to_string();
                let spell_data = self.feature_spell_data_mut().ok_or_else(|| {
                    expr::Error::unsupported_var(Attribute::FreeUsesUsed(AttrKey::named(
                        &spell_name,
                    )))
                })?;
                let spell = spell_data
                    .spells
                    .iter_mut()
                    .find(|spell| spell.name == spell_name)
                    .ok_or_else(|| {
                        expr::Error::unsupported_var(Attribute::FreeUsesUsed(AttrKey::named(
                            &spell_name,
                        )))
                    })?;
                let fu = spell.free_uses.as_mut().ok_or_else(|| {
                    expr::Error::unsupported_var(Attribute::FreeUsesUsed(AttrKey::named(
                        &spell_name,
                    )))
                })?;
                let max = fu.max as i32;
                fu.used = value.clamp(0, max) as u32;
                Ok(())
            }
            AttrKey::Scoped => {
                let new_used = value.max(0) as u32;
                let Some(spell_data) = self.feature_spell_data_mut() else {
                    return Ok(());
                };
                for spell in spell_data.spells.iter_mut() {
                    if let Some(fu) = spell.free_uses.as_mut() {
                        fu.used = new_used.min(fu.max);
                    }
                }
                Ok(())
            }
        }
    }

    /// Resolve an optional pool to concrete: explicit `Some(pool)` returns it
    /// as-is; `None` falls back to the scoped feature's `SpellData.pool`.
    fn resolve_pool(&self, pool: Option<SpellSlotPool>) -> Result<SpellSlotPool, expr::Error> {
        pool.map_or_else(|| self.feature_pool(), Ok)
    }

    fn feature_pool(&self) -> Result<SpellSlotPool, expr::Error> {
        self.feature_spell_data()
            .map(|data| data.pool)
            .ok_or_else(|| expr::Error::unsupported_var(Attribute::SlotPool))
    }

    fn feature_ability(&self) -> Result<Ability, expr::Error> {
        self.feature_spell_data()
            .map(|data| data.casting_ability)
            .ok_or_else(|| expr::Error::unsupported_var(Attribute::CasterAbility))
    }

    /// Highest N where `spell_slots[pool][N-1].total > 0`. Default 1 if all
    /// zero. Used as the level for new placeholder spells.
    fn highest_slot_level_for(character: &CharacterCore, pool: SpellSlotPool) -> u32 {
        character
            .spell_slots
            .get(&pool)
            .and_then(|levels| {
                levels
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, slot)| slot.total > 0)
                    .map(|(index, _)| (index + 1) as u32)
            })
            .unwrap_or(1)
    }

    /// Trim non-sticky entries from `target` so that those passing `keep`
    /// end up at exactly `desired_count`. Empty placeholders go first, then
    /// filled non-sticky entries in order. Sticky entries are never removed.
    fn fit_spells_to_count(
        list: &mut Vec<Spell>,
        keep: impl Fn(&Spell) -> bool,
        desired_count: u32,
        new_entry: impl Fn() -> Spell,
    ) {
        let current = list
            .iter()
            .filter(|spell| !spell.sticky && keep(spell))
            .count() as u32;
        if desired_count > current {
            for _ in 0..(desired_count - current) {
                list.push(new_entry());
            }
            return;
        }
        if desired_count == current {
            return;
        }
        let mut to_remove = (current - desired_count) as usize;
        list.retain(|spell| {
            if to_remove > 0 && !spell.sticky && keep(spell) && spell.name.is_empty() {
                to_remove -= 1;
                false
            } else {
                true
            }
        });
        if to_remove == 0 {
            return;
        }
        list.retain(|spell| {
            if to_remove > 0 && !spell.sticky && keep(spell) {
                to_remove -= 1;
                false
            } else {
                true
            }
        });
    }
}

impl expr::Context<Attribute, i32> for ApplyContext<'_> {
    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        match var {
            Attribute::HitDice => {
                if let Some(class) = self.scoped_class_mut() {
                    let max = class.level as i32;
                    class.hit_dice_used = (max - value).clamp(0, max) as u32;
                }
                Ok(())
            }
            Attribute::HitDiceUsed => {
                if let Some(class) = self.scoped_class_mut() {
                    let max = class.level as i32;
                    class.hit_dice_used = value.clamp(0, max) as u32;
                }
                Ok(())
            }
            Attribute::HitDiceSides => {
                if let Some(class) = self.scoped_class_mut() {
                    class.hit_die_sides = value.max(0) as u32;
                }
                Ok(())
            }
            Attribute::HitDiceMax => {
                log::warn!("HIT_DICE.MAX is read-only (= class level)");
                Ok(())
            }
            Attribute::Points(n) => self.assign_points(n, value),
            Attribute::PointsMax(n) => self.assign_points_max(n, value),
            Attribute::DieSides(n) => self.assign_die_sides(n, value),
            Attribute::DieCount(n) => self.assign_die_count(n, value),
            Attribute::DieUsed(n) => self.assign_die_used(n, value),
            Attribute::Bonus(n) => self.assign_bonus(n, value),
            Attribute::ChoiceCount(n) => self.assign_choice_count(n, value),
            Attribute::Sticky(n) => self.assign_sticky(n, value),
            Attribute::FreeUses(n) => self.assign_free_uses(n, value),
            Attribute::FreeUsesUsed(n) => self.assign_free_uses_used(n, value),
            Attribute::Slot(pool, n) => {
                let pool = self.resolve_pool(pool)?;
                let total = value.max(0) as u32;
                let levels = self.character.spell_slots.entry(pool).or_default();
                let slot = &mut levels[(n - 1) as usize];
                slot.total = total;
                if slot.used > total {
                    slot.used = total;
                }
                Ok(())
            }
            Attribute::SlotUsed(pool, n) => {
                let pool = self.resolve_pool(pool)?;
                let levels = self.character.spell_slots.entry(pool).or_default();
                let slot = &mut levels[(n - 1) as usize];
                slot.used = value.clamp(0, slot.total as i32) as u32;
                Ok(())
            }
            Attribute::SlotPool => {
                let new_pool = SpellSlotPool::try_from(value.max(0) as u8)
                    .map_err(|_| expr::Error::unsupported_var(Attribute::SlotPool))?;
                self.feature_spell_data_or_create().pool = new_pool;
                Ok(())
            }
            Attribute::CasterAbility => {
                let new_ability = Ability::try_from(value.max(0) as u8)
                    .map_err(|_| expr::Error::unsupported_var(Attribute::CasterAbility))?;
                self.feature_spell_data_or_create().casting_ability = new_ability;
                Ok(())
            }
            Attribute::CasterCoef => {
                self.feature_spell_data_or_create().caster_coef = value.clamp(1, 3) as u32;
                Ok(())
            }
            Attribute::SpellCantrips => {
                let data = self
                    .feature_spell_data_mut()
                    .ok_or_else(|| expr::Error::unsupported_var(Attribute::SpellCantrips))?;
                Self::fit_spells_to_count(
                    &mut data.spells,
                    |spell| spell.level == 0,
                    value.max(0) as u32,
                    || Spell {
                        level: 0,
                        ..Default::default()
                    },
                );
                Ok(())
            }
            Attribute::SpellReady => {
                let pool = self.feature_pool()?;
                let highest = Self::highest_slot_level_for(self.character, pool);
                let data = self
                    .feature_spell_data_mut()
                    .ok_or_else(|| expr::Error::unsupported_var(Attribute::SpellReady))?;
                Self::fit_spells_to_count(
                    &mut data.spells,
                    |spell| spell.level > 0,
                    value.max(0) as u32,
                    || Spell {
                        level: highest,
                        ..Default::default()
                    },
                );
                Ok(())
            }
            Attribute::Subclass(name) => {
                let class_name_opt = self.scoped_class().map(|cl| cl.class.clone());
                if let Some(class) = self.scoped_class_mut() {
                    if value != 0 {
                        class.subclass = Some(name.into());
                    } else if class.subclass.as_deref() == Some(name) {
                        class.subclass = None;
                    }
                }
                if value != 0
                    && let Some(class_name) = class_name_opt
                {
                    self.identity_changes.push(IdentityChange::Subclass {
                        class: class_name,
                        name: name.into(),
                    });
                }
                Ok(())
            }
            Attribute::ClassLevel(AttrKey::Named(name)) => {
                let old_level = self
                    .character
                    .identity
                    .classes
                    .iter()
                    .find(|cl| &*cl.class == name)
                    .map(|cl| cl.level)
                    .unwrap_or(0);
                self.character.assign(var, value)?;
                let new_level = self
                    .character
                    .identity
                    .classes
                    .iter()
                    .find(|cl| &*cl.class == name)
                    .map(|cl| cl.level)
                    .unwrap_or(0);
                if new_level > old_level {
                    self.identity_changes.push(IdentityChange::ClassLevel {
                        class: name.into(),
                        old: old_level,
                        new: new_level,
                    });
                }
                Ok(())
            }
            Attribute::Species(name) => {
                let changed = self.character.identity.species.as_str() != name;
                self.character.assign(var, value)?;
                if changed && !self.character.identity.species.is_empty() {
                    self.identity_changes
                        .push(IdentityChange::Species(name.into()));
                }
                Ok(())
            }
            Attribute::Background(name) => {
                let changed = self.character.identity.background.as_str() != name;
                self.character.assign(var, value)?;
                if changed && !self.character.identity.background.is_empty() {
                    self.identity_changes
                        .push(IdentityChange::Background(name.into()));
                }
                Ok(())
            }
            Attribute::SpellKnown => {
                let pool = self.feature_pool()?;
                let highest = Self::highest_slot_level_for(self.character, pool);
                let data = self
                    .feature_spell_data_mut()
                    .ok_or_else(|| expr::Error::unsupported_var(Attribute::SpellKnown))?;
                let known = data.known.get_or_insert_with(Vec::new);
                Self::fit_spells_to_count(
                    known,
                    |_| true,
                    value.max(0) as u32,
                    || Spell {
                        level: highest,
                        ..Default::default()
                    },
                );
                Ok(())
            }
            _ => self.character.assign(var, value),
        }
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::ClassLevel(AttrKey::Scoped) => Ok(self.class_level()),
            Attribute::HitDice => Ok(self
                .scoped_class()
                .map(|cl| (cl.level as i32 - cl.hit_dice_used as i32).max(0))
                .unwrap_or(0)),
            Attribute::HitDiceMax => Ok(self.scoped_class().map(|cl| cl.level as i32).unwrap_or(0)),
            Attribute::HitDiceUsed => Ok(self
                .scoped_class()
                .map(|cl| cl.hit_dice_used as i32)
                .unwrap_or(0)),
            Attribute::HitDiceSides => Ok(self
                .scoped_class()
                .map(|cl| cl.hit_die_sides as i32)
                .unwrap_or(0)),
            Attribute::Arg(n) => self
                .current_feature()
                .inputs
                .get(self.expr_index)
                .and_then(|inputs| inputs.args.get(n as usize))
                .copied()
                .ok_or_else(|| expr::Error::unsupported_var(var)),
            Attribute::CasterLevel(None) => {
                let pool = self.feature_pool()?;
                Ok(self.character.caster_level(pool) as i32)
            }
            Attribute::CasterLevel(Some(pool)) => Ok(self.character.caster_level(pool) as i32),
            Attribute::CasterModifier => {
                let ability = self.feature_ability()?;
                Ok(self.character.ability_modifier(ability))
            }
            Attribute::Points(n) => self.resolve_points(n),
            Attribute::PointsMax(n) => self.resolve_points_max(n),
            Attribute::DieSides(n) => self.resolve_die_sides(n),
            Attribute::DieCount(n) => self.resolve_die_count(n),
            Attribute::DieUsed(n) => self.resolve_die_used(n),
            Attribute::Bonus(n) => self.resolve_bonus(n),
            Attribute::ChoiceCount(n) => self.resolve_choice_count(n),
            Attribute::Sticky(n) => self.resolve_sticky(n),
            Attribute::FreeUses(n) => self.resolve_free_uses(n),
            Attribute::FreeUsesUsed(n) => self.resolve_free_uses_used(n),
            Attribute::Slot(pool, n) => {
                let pool = self.resolve_pool(pool)?;
                Ok(self.character.spell_slots.get_slot(pool, n as u32).total as i32)
            }
            Attribute::SlotUsed(pool, n) => {
                let pool = self.resolve_pool(pool)?;
                Ok(self.character.spell_slots.get_slot(pool, n as u32).used as i32)
            }
            Attribute::SlotPool => Ok(self
                .feature_spell_data()
                .ok_or_else(|| expr::Error::unsupported_var(Attribute::SlotPool))?
                .pool as i32),
            Attribute::CasterAbility => Ok(self
                .feature_spell_data()
                .ok_or_else(|| expr::Error::unsupported_var(Attribute::CasterAbility))?
                .casting_ability as i32),
            Attribute::CasterCoef => Ok(self
                .feature_spell_data()
                .ok_or_else(|| expr::Error::unsupported_var(Attribute::CasterCoef))?
                .caster_coef as i32),
            Attribute::SpellCantrips => {
                let data = self
                    .feature_spell_data()
                    .ok_or_else(|| expr::Error::unsupported_var(Attribute::SpellCantrips))?;
                Ok(data
                    .spells
                    .iter()
                    .filter(|spell| !spell.sticky && spell.level == 0)
                    .count() as i32)
            }
            Attribute::SpellReady => {
                let data = self
                    .feature_spell_data()
                    .ok_or_else(|| expr::Error::unsupported_var(Attribute::SpellReady))?;
                Ok(data
                    .spells
                    .iter()
                    .filter(|spell| !spell.sticky && spell.level > 0)
                    .count() as i32)
            }
            Attribute::SpellKnown => {
                let data = self
                    .feature_spell_data()
                    .ok_or_else(|| expr::Error::unsupported_var(Attribute::SpellKnown))?;
                Ok(data
                    .known
                    .as_deref()
                    .map(|known: &[Spell]| known.iter().filter(|spell| !spell.sticky).count())
                    .unwrap_or(0) as i32)
            }
            _ => self.character.resolve(var),
        }
    }
}

/// Iterate `assignments` against a generic `Context` that doesn't carry
/// feature-scope itself, wrapping each interactive assignment in
/// [`crate::rules::apply::args_ctx::WithArgs`] so `@ARG(n)` lookups
/// resolve from the matching `inputs` slot. Used by the preview panel
/// where the inner context (`PreviewContext`) intercepts `assign` calls
/// for display rather than producing real character mutations.
pub fn apply_assignments_with_inputs<C: expr::Context<Attribute, i32>>(
    ctx: &mut C,
    assignments: &[Assignment],
    when: WhenCondition,
    inputs: &[AssignInputs],
    log_errors: bool,
) {
    let mut input_iter = inputs.iter();
    for assign in assignments.iter().filter(|a| a.when == when) {
        let input = assign.is_interactive().then(|| input_iter.next()).flatten();
        let result = if let Some(input) = input
            && !input.is_empty()
        {
            let mut wrapped = WithArgs {
                inner: ctx,
                args: &input.args,
            };
            if input.dice.is_empty() {
                assign.expr.apply(&mut wrapped)
            } else {
                assign.expr.apply_with_dice(&mut wrapped, &input.dice)
            }
        } else {
            assign.expr.apply(ctx)
        };
        if log_errors && let Err(error) = result {
            log::error!("Failed to apply assignment: {error:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::{
        expr::Context as _,
        model::{Character, ClassLevel, Feature, FeatureSource, intern},
    };

    fn fighter_at_5() -> Character {
        let mut character = Character::new();
        character.identity.classes.push(ClassLevel {
            class: "Fighter".into(),
            level: 5,
            hit_die_sides: 10,
            hit_dice_used: 2,
            ..ClassLevel::default()
        });
        character.features.list.push(Feature {
            name: "Second Wind".into(),
            source: FeatureSource::Class("Fighter".into(), 1),
            ..Default::default()
        });
        character
    }

    #[wasm_bindgen_test]
    fn hit_dice_resolves_through_class_scope() {
        let mut character = fighter_at_5();
        let ctx = ApplyContext::new(&mut character, 0);

        assert_eq!(ctx.resolve(Attribute::HitDiceMax).unwrap(), 5);
        assert_eq!(ctx.resolve(Attribute::HitDiceUsed).unwrap(), 2);
        assert_eq!(ctx.resolve(Attribute::HitDice).unwrap(), 3);
        assert_eq!(ctx.resolve(Attribute::HitDiceSides).unwrap(), 10);
    }

    #[wasm_bindgen_test]
    fn hit_dice_used_assign_writes_back() {
        let mut character = fighter_at_5();
        let mut ctx = ApplyContext::new(&mut character, 0);

        ctx.assign(Attribute::HitDiceUsed, 0).unwrap();
        assert_eq!(ctx.resolve(Attribute::HitDiceUsed).unwrap(), 0);

        // HIT_DICE = 4 → used = max - 4 = 1
        ctx.assign(Attribute::HitDice, 4).unwrap();
        assert_eq!(ctx.resolve(Attribute::HitDiceUsed).unwrap(), 1);
        assert_eq!(ctx.resolve(Attribute::HitDice).unwrap(), 4);

        // Clamps to [0, max]
        ctx.assign(Attribute::HitDiceUsed, 99).unwrap();
        assert_eq!(ctx.resolve(Attribute::HitDiceUsed).unwrap(), 5);
    }

    #[wasm_bindgen_test]
    fn hit_dice_sides_assign_writes_back() {
        // Class proficiencies feature can set the die size:
        //   HIT_DICE.SIDES = 10
        let mut character = fighter_at_5();
        // Wipe the field so we observe the assign actually writing.
        character.identity.classes[0].hit_die_sides = 0;
        let mut ctx = ApplyContext::new(&mut character, 0);

        ctx.assign(Attribute::HitDiceSides, 10).unwrap();
        assert_eq!(ctx.resolve(Attribute::HitDiceSides).unwrap(), 10);
    }

    #[wasm_bindgen_test]
    fn hit_dice_zero_for_non_class_features() {
        let mut character = Character::new();
        character.features.list.push(Feature {
            name: "Common".into(),
            source: FeatureSource::Species("Human".into()),
            ..Default::default()
        });
        let ctx = ApplyContext::new(&mut character, 0);

        // Non-Class scope → no class lookup → 0.
        assert_eq!(ctx.resolve(Attribute::HitDice).unwrap(), 0);
        assert_eq!(ctx.resolve(Attribute::HitDiceMax).unwrap(), 0);
        assert_eq!(ctx.resolve(Attribute::HitDiceUsed).unwrap(), 0);
        assert_eq!(ctx.resolve(Attribute::HitDiceSides).unwrap(), 0);
    }

    #[wasm_bindgen_test]
    fn assign_records_class_level_event() {
        let mut character = Character::new();
        character.features.list.push(Feature {
            name: "Wizard".into(),
            source: FeatureSource::User(1),
            ..Default::default()
        });
        let mut ctx = ApplyContext::new(&mut character, 0);
        ctx.assign(Attribute::ClassLevel(AttrKey::Named(intern("Wizard"))), 1)
            .unwrap();
        let events = ctx.take_identity_changes();
        assert_eq!(events.len(), 1);
        match &events[0] {
            IdentityChange::ClassLevel { class, old, new } => {
                assert_eq!(class.as_ref(), "Wizard");
                assert_eq!(*old, 0);
                assert_eq!(*new, 1);
            }
            other => panic!("expected ClassLevel event, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    fn assign_records_subclass_event() {
        let mut character = Character::new();
        character.identity.classes.push(ClassLevel {
            class: "Fighter".into(),
            level: 3,
            ..ClassLevel::default()
        });
        character.features.list.push(Feature {
            name: "Battle Master".into(),
            source: FeatureSource::Class("Fighter".into(), 3),
            ..Default::default()
        });
        let mut ctx = ApplyContext::new(&mut character, 0);
        ctx.assign(Attribute::Subclass(intern("Battle Master")), 1)
            .unwrap();
        let events = ctx.take_identity_changes();
        assert_eq!(events.len(), 1);
        match &events[0] {
            IdentityChange::Subclass { class, name } => {
                assert_eq!(class.as_ref(), "Fighter");
                assert_eq!(name.as_ref(), "Battle Master");
            }
            other => panic!("expected Subclass event, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    fn assign_records_species_and_background_events() {
        let mut character = Character::new();
        character.features.list.push(Feature {
            name: "Tag".into(),
            source: FeatureSource::User(0),
            ..Default::default()
        });
        let mut ctx = ApplyContext::new(&mut character, 0);
        ctx.assign(Attribute::Species(intern("Wood Elf")), 1)
            .unwrap();
        ctx.assign(Attribute::Background(intern("Sage")), 1)
            .unwrap();
        let events = ctx.take_identity_changes();
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], IdentityChange::Species(name) if name.as_ref() == "Wood Elf"));
        assert!(matches!(&events[1], IdentityChange::Background(name) if name.as_ref() == "Sage"));
    }
}

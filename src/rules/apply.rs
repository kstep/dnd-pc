use leptos::prelude::*;

use super::{
    WhenCondition,
    resolve::{find_feature, find_feature_with_class_level},
    spells::SpellList,
};
use crate::{
    model::{Character, Context, FeatureSource},
    rules::RulesRegistry,
};

impl RulesRegistry {
    pub fn long_rest(&self, character: &mut Character) {
        character.long_rest();
        self.assign(character, WhenCondition::OnLongRest);
    }

    pub fn short_rest(&self, character: &mut Character) {
        character.short_rest();
        self.assign(character, WhenCondition::OnShortRest);
    }

    pub fn compute(&self, character: &mut Character) {
        character.compute();
        self.assign(character, WhenCondition::OnCompute);
    }

    /// Evaluate assignment expressions across all racial traits and features
    /// for the given condition.
    ///
    /// Racial traits are evaluated first (flat Character context), then
    /// features with per-feature `Context` providing `CLASS_LEVEL`,
    /// `CASTER_LEVEL`, and `CASTER_MODIFIER`.
    pub fn assign(&self, character: &mut Character, when: WhenCondition) {
        let class_cache = self.class_cache.read_untracked();
        let bg_cache = self.background_cache.read_untracked();
        let race_cache = self.race_cache.read_untracked();

        // Racial traits first (e.g. speed override, Dwarf Toughness)
        let trait_exprs: Vec<_> = race_cache
            .get(character.identity.race.as_str())
            .into_iter()
            .flat_map(|race_def| race_def.traits.values())
            .filter_map(|racial_trait| racial_trait.assign.as_ref())
            .flat_map(|assignments| assignments.iter())
            .filter(|a| a.when == when)
            .map(|a| a.expr.clone())
            .collect();

        for expr in trait_exprs {
            if let Err(error) = expr.apply(character) {
                log::error!("Failed to apply trait assignment: {error:?}");
            }
        }

        // Collect per-feature info: (expressions, class_level, caster_level,
        // caster_modifier). Uses find_feature_with_class_level for a single-pass
        // lookup. The Vec is necessary because we need &mut character in the loop.
        let feature_entries: Vec<_> = character
            .features
            .iter()
            .filter_map(|feat| {
                let (feat_def, class_level) = find_feature_with_class_level(
                    &character.identity,
                    &feat.name,
                    &class_cache,
                    &bg_cache,
                    &race_cache,
                )?;
                let exprs: Vec<_> = feat_def
                    .assign
                    .iter()
                    .flat_map(|a| a.iter())
                    .filter(|a| a.when == when)
                    .map(|a| a.expr.clone())
                    .collect();
                if exprs.is_empty() {
                    return None;
                }
                let (caster_level, caster_modifier) = feat_def
                    .spells
                    .as_ref()
                    .map(|s| {
                        (
                            character.caster_level(s.pool) as i32,
                            character.ability_modifier(s.casting_ability),
                        )
                    })
                    .unwrap_or((0, 0));
                Some((exprs, class_level as i32, caster_level, caster_modifier))
            })
            .collect();

        for (exprs, class_level, caster_level, caster_modifier) in feature_entries {
            let mut ctx = Context {
                character,
                class_level,
                caster_level,
                caster_modifier,
            };
            for expr in exprs {
                if let Err(error) = expr.apply(&mut ctx) {
                    log::error!("Failed to apply assignment: {error:?}");
                }
            }
        }
    }

    /// Apply class level-up logic. Applies all unapplied levels from last
    /// applied+1 through the current class level. Handles saving throws,
    /// proficiencies, spell slots, class/race/background features, and HP.
    pub fn apply_class_level(&self, character: &mut Character, class_idx: usize, level: u32) {
        let Some(class_level) = character.identity.classes.get_mut(class_idx) else {
            return;
        };

        if class_level.applied_levels.contains(&level) {
            return;
        }

        let class_cache = self.class_cache.read_untracked();
        let Some(def) = class_cache.get(class_level.class.as_str()) else {
            return;
        };

        // Mark level as applied only after confirming the definition is loaded
        class_level.applied_levels.insert(level);
        let subclass = class_level.subclass.clone();
        let source = FeatureSource::Class(def.name.clone());

        // Record applied level and set hit die
        class_level.hit_die_sides = def.hit_die;

        // Apply saving throws and proficiencies
        character.update_saving_throw_proficiencies(|saving_throws| {
            saving_throws.extend(def.saving_throws.iter().copied());
        });
        character.update_proficiencies(|proficiencies| {
            proficiencies.extend(def.proficiencies.iter().cloned());
        });

        // Apply class features: create SpellData, update slots, apply features
        let rules = def.levels.get(level as usize - 1);
        let subclass_rules = subclass
            .as_deref()
            .and_then(|sc| def.subclasses.get(sc))
            .and_then(|sc| sc.levels.get(&level));

        for feat in def.features(subclass.as_deref()) {
            let is_new = rules.is_some_and(|r| r.features.contains(&feat.name))
                || subclass_rules.is_some_and(|r| r.features.contains(&feat.name));
            let already_has = character.features.iter().any(|f| f.name == feat.name);

            if is_new && already_has && !feat.stackable {
                continue;
            }

            if is_new || already_has {
                feat.apply(level, character, &source);
            }
        }

        // Re-apply race and background features at new total level
        // (unlocks level-gated spells, e.g. Tiefling's Infernal Legacy)
        let total_level = character.level();

        let race_cache = self.race_cache.read_untracked();
        if let Some(race_def) = race_cache.get(character.identity.race.as_str()) {
            let source = FeatureSource::Race(character.identity.race.clone());
            for feat in race_def.features.values() {
                feat.apply(total_level, character, &source);
            }
        }

        let bg_cache = self.background_cache.read_untracked();
        if let Some(bg_def) = bg_cache.get(character.identity.background.as_str()) {
            let source = FeatureSource::Background(character.identity.background.clone());
            for feat in bg_def.features.values() {
                feat.apply(total_level, character, &source);
            }
        }

        // Recompute all derived stats (HP, AC, speed) + OnCompute assignments
        let old_hp_max = character.hp_max();
        self.compute(character);
        let hp_delta = character.hp_max().saturating_sub(old_hp_max);
        character.combat.hp_current += hp_delta;
    }

    /// Trigger spell list fetches for all feature data entries that reference
    /// external spell lists. Used by `fill_from_registry` before acquiring
    /// the spell list cache read guard.
    pub(super) fn trigger_spell_list_fetches(&self, character: &Character) {
        let class_cache = self.class_cache.read_untracked();
        let bg_cache = self.background_cache.read_untracked();
        let race_cache = self.race_cache.read_untracked();

        for key in character.feature_data.keys() {
            if let Some(feat_def) = find_feature(
                &character.identity,
                key,
                &class_cache,
                &bg_cache,
                &race_cache,
            ) && let Some(spells_def) = &feat_def.spells
                && let SpellList::Ref { from } = &spells_def.list
            {
                self.fetch_spell_list(from);
            }
        }
    }
}

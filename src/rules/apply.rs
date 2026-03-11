use leptos::prelude::*;

use super::{WhenCondition, resolve::find_feature, spells::SpellList};
use crate::{
    model::{Ability, Character, FeatureSource},
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

    /// Evaluate assignment expressions across all features and racial traits
    /// for rest/global events.
    ///
    /// This intentionally uses a simpler evaluation path than
    /// `FeatureDefinition::assign` (which uses a `Context` with class-level
    /// awareness). Rest-time expressions operate on the full character without
    /// per-feature class-level context, since rest effects are class-agnostic.
    pub fn assign(&self, character: &mut Character, when: WhenCondition) {
        let class_cache = self.class_cache.read_untracked();
        let bg_cache = self.background_cache.read_untracked();
        let race_cache = self.race_cache.read_untracked();

        let feature_exprs = character
            .features
            .iter()
            .filter_map(|feat| {
                let feat_def = find_feature(
                    &character.identity,
                    &feat.name,
                    &class_cache,
                    &bg_cache,
                    &race_cache,
                )?;
                Some(
                    feat_def
                        .assign
                        .iter()
                        .flat_map(|a| a.iter())
                        .filter(|a| a.when == when)
                        .map(|a| a.expr.clone())
                        .collect::<Vec<_>>(),
                )
            })
            .flatten();

        let trait_exprs = race_cache
            .get(character.identity.race.as_str())
            .into_iter()
            .flat_map(|race_def| race_def.traits.values())
            .filter_map(|racial_trait| racial_trait.assign.as_ref())
            .flat_map(|assignments| assignments.iter())
            .filter(|a| a.when == when)
            .map(|a| a.expr.clone());

        let exprs: Vec<_> = feature_exprs.chain(trait_exprs).collect();

        for expr in exprs {
            if let Err(error) = expr.apply(character) {
                log::error!("Failed to apply rest assignment: {error:?}");
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
        character
            .saving_throws
            .extend(def.saving_throws.iter().copied());
        character
            .proficiencies
            .extend(def.proficiencies.iter().copied());

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

        // Apply HP gain
        let con_mod = character.ability_modifier(Ability::Constitution);
        let hp_gain = if level == 1 {
            def.hit_die as i32 + con_mod
        } else {
            (def.hit_die as i32) / 2 + 1 + con_mod
        };
        character.combat.hp_max = character
            .combat
            .hp_max
            .saturating_add_signed(hp_gain.max(1));
        character.combat.hp_current = character.combat.hp_max;

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

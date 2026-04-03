use std::collections::BTreeMap;

use super::{
    class::ClassDefinition,
    feature::{FeatureDefinition, FieldKind},
    resolve::find_feature,
    spells::{SpellDefinition, SpellList, SpellMap},
};
use crate::model::{Character, Spell, SpellData};

/// Resolve a spell definition from an inline list or a cached reference list.
/// O(log n) lookup via BTreeMap.
fn resolve_spell_def<'a>(
    list: &'a SpellList,
    spell_list_cache: &'a BTreeMap<Box<str>, SpellMap>,
    spell_name: &str,
) -> Option<&'a SpellDefinition> {
    match list {
        SpellList::Inline(spells) => spells.get(spell_name),
        SpellList::Ref { from } => spell_list_cache
            .get(from.as_str())
            .and_then(|v| v.get(spell_name)),
    }
}

/// Single traversal for both fill and clear operations.
/// Closures determine the leaf behavior (fill-if-empty vs clear-if-matches).
///
/// `set_label`: applied to every `Option<String>` label paired with a
///     definition `Option<String>` source.
/// `set_desc`: applied to every `String` description paired with a definition
///     `&str` source.
/// `on_spell_extra`: called per spell with the matching definition (if found)
///     and the `free_uses_max` resolved from the owning feature's `FreeUses`
///     field kind. Fill uses this to set `cost` / `free_uses`; clear uses it
///     to zero them.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_labels(
    character: &mut Character,
    class_cache: &BTreeMap<Box<str>, ClassDefinition>,
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
    spell_list_cache: &BTreeMap<Box<str>, SpellMap>,
    mut set_label: impl FnMut(&mut Option<String>, Option<&str>),
    mut set_desc: impl FnMut(&mut String, &str),
    mut on_spell_extra: impl FnMut(&mut Spell, Option<&SpellDefinition>, u32),
) {
    // 1. Class/subclass labels
    for cl in &mut character.identity.classes {
        if cl.class.is_empty() {
            continue;
        }
        if let Some(def) = class_cache.get(cl.class.as_str()) {
            set_label(&mut cl.class_label, def.label.as_deref());
            if let Some(sc_name) = &cl.subclass
                && let Some(sc_def) = def.subclasses.get(sc_name.as_str())
            {
                set_label(&mut cl.subclass_label, sc_def.label.as_deref());
            }
        }
    }

    // 2. Feature labels/descriptions
    for feature in &mut character.features {
        if feature.name.is_empty() {
            continue;
        }
        if let Some(feat_def) = find_feature(&feature.name, features_index) {
            set_label(&mut feature.label, feat_def.label.as_deref());
            set_desc(&mut feature.description, &feat_def.description);
            feature.category = feat_def.category;
        }
    }

    // 3. Feature data: fields, choices, spells
    let char_level = character.level();

    // Pre-compute free_uses_max for each feature (needs immutable character
    // borrow, which conflicts with the mutable feature_data iteration below).
    let free_uses_map: BTreeMap<String, u32> = character
        .feature_data
        .keys()
        .filter_map(|key| {
            let feat_def = find_feature(key, features_index)?;
            let max = feat_def.free_uses_max(char_level, character);
            (max > 0).then(|| (key.clone(), max))
        })
        .collect();

    for (key, entry) in &mut character.feature_data {
        let Some(feat_def) = find_feature(key, features_index) else {
            continue;
        };

        // Field labels/descriptions + choice option labels/descriptions
        for field in &mut entry.fields {
            if let Some(field_def) = feat_def.fields.get(field.name.as_str()) {
                set_label(&mut field.label, field_def.label.as_deref());
                set_desc(&mut field.description, &field_def.description);

                if let FieldKind::Choice { options, .. } = &field_def.kind {
                    let def_options = feat_def.resolve_def_options(options);
                    for opt in field.value.choices_mut() {
                        if opt.name.is_empty() {
                            continue;
                        }
                        if let Some(def_opt) = def_options.iter().find(|o| o.name == opt.name) {
                            set_label(&mut opt.label, def_opt.label.as_deref());
                            set_desc(&mut opt.description, &def_opt.description);
                        }
                    }
                }
            }
        }

        // Spell labels/descriptions + extra per-spell processing
        if let Some(spells_def) = &feat_def.spells
            && let Some(spell_data) = &mut entry.spells
        {
            let free_uses_max = free_uses_map.get(key.as_str()).copied().unwrap_or(0);

            // Sync spellbook (known) labels from registry first
            if let Some(known) = &mut spell_data.known {
                for spell in known.iter_mut() {
                    if spell.name.is_empty() {
                        continue;
                    }
                    let spell_def =
                        resolve_spell_def(&spells_def.list, spell_list_cache, &spell.name);
                    if let Some(def) = spell_def {
                        set_label(&mut spell.label, def.label.as_deref());
                        set_desc(&mut spell.description, &def.description);
                    }
                }
            }

            let SpellData { known, spells, .. } = spell_data;
            let known_spells = known.as_deref().unwrap_or_default();
            for spell in spells.iter_mut() {
                if spell.name.is_empty() {
                    continue;
                }
                // Two-tier: fill from spellbook entry first, registry fallback
                let spell_def = resolve_spell_def(&spells_def.list, spell_list_cache, &spell.name);
                let known_entry = known_spells.iter().find(|s| s.name == spell.name);
                if let Some(known) = known_entry {
                    set_label(&mut spell.label, known.label.as_deref());
                    set_desc(&mut spell.description, &known.description);
                } else if let Some(def) = spell_def {
                    set_label(&mut spell.label, def.label.as_deref());
                    set_desc(&mut spell.description, &def.description);
                }
                on_spell_extra(spell, spell_def, free_uses_max);
            }
        }
    }
}

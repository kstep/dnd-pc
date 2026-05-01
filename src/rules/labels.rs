use std::collections::BTreeMap;

use leptos::prelude::*;

use crate::{
    model::{Character, FeatureField, FeatureOption, FeatureValue, Spell},
    rules::{
        cache::FetchCache,
        feature::{ChoiceOptions, FeatureDefinition, FeaturesIndex},
        locale::{LocaleKey, LocaleMap, SpellsLocaleMap},
        registry::LocalizedIndex,
        spells::SpellsIndex,
    },
};

/// Per-spell extras the caller wants to either fill from the feature's spell
/// entry or clear. `cost` comes from the matching `SpellEntry` (or 0 if the
/// feature uses a `Ref { from }` block — those have no per-entry cost).
/// `level` comes from the global `SpellsIndex` (catalog enrichment).
pub(super) struct SpellExtras {
    pub cost: u32,
    pub level: Option<u32>,
}

/// Single traversal for both fill and clear operations.
/// Closures determine the leaf behavior (fill-if-empty vs clear-if-matches).
#[allow(clippy::too_many_arguments)]
#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "labels.sync_labels", skip_all)
)]
pub(super) fn sync_labels(
    character: &mut Character,
    class_locale_cache: FetchCache<LocaleMap>,
    features_index: LocalizedIndex<FeaturesIndex, LocaleMap>,
    spells_index: LocalizedIndex<SpellsIndex, SpellsLocaleMap>,
    mut set_label: impl FnMut(&mut Option<String>, Option<&str>),
    mut set_desc: impl FnMut(&mut String, &str),
    mut on_spell_extra: impl FnMut(&mut Spell, SpellExtras),
) {
    // 1. Class/subclass labels
    let class_locales = class_locale_cache.read_untracked();
    for class_level in &mut character.identity.classes {
        if class_level.class.is_empty() {
            continue;
        }
        if let Some(map) = class_locales.get(class_level.class.as_str()) {
            let class_text = map.get("");
            set_label(
                &mut class_level.class_label,
                class_text.and_then(|t| t.label.as_deref()),
            );
            if let Some(sub_name) = &class_level.subclass {
                let sub_key = format!("subclass.{sub_name}");
                let sub_text = map.get(sub_key.as_str());
                set_label(
                    &mut class_level.subclass_label,
                    sub_text.and_then(|t| t.label.as_deref()),
                );
            }
        }
    }
    drop(class_locales);

    features_index.with_untracked(|index, locale| {
        sync_feature_labels(
            character,
            &index.0,
            locale,
            spells_index,
            &mut set_label,
            &mut set_desc,
            &mut on_spell_extra,
        );
    });
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "labels.sync_feature_labels", skip_all)
)]
fn sync_feature_labels(
    character: &mut Character,
    features_map: &BTreeMap<Box<str>, FeatureDefinition>,
    raw_locale: Option<&LocaleMap>,
    spells_index: LocalizedIndex<SpellsIndex, SpellsLocaleMap>,
    set_label: &mut impl FnMut(&mut Option<String>, Option<&str>),
    set_desc: &mut impl FnMut(&mut String, &str),
    on_spell_extra: &mut impl FnMut(&mut Spell, SpellExtras),
) {
    // 2. Feature labels/descriptions
    for feature in &mut character.features {
        if feature.name.is_empty() {
            continue;
        }
        if let Some(feat_def) = features_map.get(feature.name.as_str()) {
            let text = raw_locale.and_then(|m| m.get(&*feat_def.name));
            set_label(&mut feature.label, text.and_then(|t| t.label.as_deref()));
            set_desc(
                &mut feature.description,
                text.and_then(|t| t.description.as_deref()).unwrap_or(""),
            );
            feature.category = feat_def.category;
        }
    }

    // 3. Feature data: fields, choices, spells
    let char_level = character.level();
    for (key, entry) in character.features.data_mut() {
        let Some(feat_def) = features_map.get(key.as_str()) else {
            continue;
        };

        // Mirror action-menu options into runtime Choice fields so the
        // renderers (build tab, session) read them from `entry.fields`
        // without peeking into `feat_def.actions`. Re-syncs each cycle:
        // labels follow locale, level-gated entries follow current level.
        for (action_name, action_def) in &feat_def.actions {
            let ChoiceOptions::List(def_opts) = &action_def.options else {
                continue;
            };
            let mirrored: Vec<FeatureOption> = def_opts
                .iter()
                .filter(|opt| opt.action.is_some() && opt.level <= char_level)
                .map(|opt| FeatureOption {
                    name: opt.name.to_string(),
                    label: None,
                    description: String::new(),
                    cost: opt.cost,
                    action: opt.action,
                })
                .collect();
            if mirrored.is_empty() {
                continue;
            }
            let pos = entry
                .fields
                .iter()
                .position(|field| field.name.as_str() == action_name.as_ref());
            let field = match pos {
                Some(idx) => &mut entry.fields[idx],
                None => {
                    entry.fields.push(FeatureField {
                        name: action_name.to_string(),
                        label: None,
                        description: String::new(),
                        value: FeatureValue::Choice {
                            options: Vec::new(),
                        },
                    });
                    entry.fields.last_mut().unwrap()
                }
            };
            field.value = FeatureValue::Choice { options: mirrored };
        }

        // Field labels/descriptions: applies to all FeatureFields, including
        // those created lazily by assign expressions (no FieldDefinition).
        for field in &mut entry.fields {
            let field_key = LocaleKey::flat_field(&feat_def.name, &field.name);
            let field_text = raw_locale.and_then(|m| m.get(field_key.as_str()));
            set_label(
                &mut field.label,
                field_text.and_then(|t| t.label.as_deref()),
            );
            set_desc(
                &mut field.description,
                field_text
                    .and_then(|t| t.description.as_deref())
                    .unwrap_or(""),
            );

            // Choice option enrichment requires a matching ActionDefinition.
            if let Some(action_def) = feat_def.actions.get(field.name.as_str()) {
                let def_options = feat_def.resolve_def_options(&action_def.options);
                for opt in field.value.choices_mut() {
                    if opt.name.is_empty() {
                        continue;
                    }
                    if let Some(def_opt) = def_options.iter().find(|def| *def.name == opt.name) {
                        let opt_key = LocaleKey::flat_field_option(
                            &feat_def.name,
                            &field.name,
                            &def_opt.name,
                        );
                        let opt_text = raw_locale.and_then(|m| m.get(opt_key.as_str()));
                        set_label(&mut opt.label, opt_text.and_then(|t| t.label.as_deref()));
                        set_desc(
                            &mut opt.description,
                            opt_text
                                .and_then(|t| t.description.as_deref())
                                .unwrap_or(""),
                        );
                    }
                }
            }
        }

        // Spell labels/descriptions + extra per-spell processing.
        // Sticky spells (post-Stage-II) live in `entry.spells` even when the
        // catalog `feat_def.spells` block was dropped — sync runs on the
        // runtime data alone, with the catalog used only for `cost` lookups.
        let Some(spell_data) = &mut entry.spells else {
            continue;
        };

        if let Some(known) = &mut spell_data.known {
            for spell in known.iter_mut() {
                if spell.name.is_empty() {
                    continue;
                }
                spells_index.lookup_untracked(spell.name.as_str(), |loc| {
                    set_label(
                        &mut spell.label,
                        loc.locale.and_then(|t| t.label.as_deref()),
                    );
                    set_desc(&mut spell.description, loc.description());
                });
            }
        }

        let known_spells = spell_data.known.as_deref().unwrap_or_default();
        for spell in spell_data.spells.iter_mut() {
            if spell.name.is_empty() {
                continue;
            }
            let known_entry = known_spells.iter().find(|known| known.name == spell.name);
            let mut catalog_level: Option<u32> = None;
            if let Some(known) = known_entry {
                set_label(&mut spell.label, known.label.as_deref());
                set_desc(&mut spell.description, &known.description);
                catalog_level = Some(known.level);
            } else {
                spells_index.lookup_untracked(spell.name.as_str(), |loc| {
                    set_label(
                        &mut spell.label,
                        loc.locale.and_then(|t| t.label.as_deref()),
                    );
                    set_desc(&mut spell.description, loc.description());
                    catalog_level = Some(loc.level);
                });
            }
            let cost = feat_def
                .spells
                .as_ref()
                .and_then(|spells_def| {
                    spells_def
                        .list
                        .inline_entries()
                        .iter()
                        .find(|entry| *entry.name == spell.name)
                        .map(|entry| entry.cost)
                })
                .unwrap_or(0);
            on_spell_extra(
                spell,
                SpellExtras {
                    cost,
                    level: catalog_level,
                },
            );
        }
    }
}

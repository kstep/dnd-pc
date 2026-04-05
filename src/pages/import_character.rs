use std::collections::BTreeSet;

use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::{
    components::A,
    hooks::{use_navigate, use_params},
    params::Params,
};
use strum::IntoEnumIterator;
use uuid::Uuid;

use crate::{
    BASE_URL, firebase,
    model::{Ability, Character, Item, Proficiency, Skill, Translatable},
    share, storage,
};

// --- Diff computation ---

struct DiffRow {
    section: &'static str,
    label: &'static str,
    local: String,
    imported: String,
}

fn push_if_diff(
    rows: &mut Vec<DiffRow>,
    section: &'static str,
    label: &'static str,
    local: String,
    imported: String,
) {
    if local != imported {
        rows.push(DiffRow {
            section,
            label,
            local,
            imported,
        });
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{truncated}...")
    }
}

fn format_names<T>(items: &[T], name_fn: impl Fn(&T) -> &str) -> String {
    let names: Vec<&str> = items
        .iter()
        .map(&name_fn)
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        "\u{2014}".to_string()
    } else {
        format!("{} ({})", names.len(), names.join(", "))
    }
}

fn format_items(items: &[Item]) -> String {
    let entries: Vec<String> = items
        .iter()
        .filter(|item| !item.name.is_empty())
        .map(|item| item.to_string())
        .collect();
    if entries.is_empty() {
        "\u{2014}".to_string()
    } else {
        format!("{} ({})", entries.len(), entries.join(", "))
    }
}

fn format_spell_slots(ch: &Character, i18n: leptos_fluent::I18n) -> String {
    let mut parts: Vec<String> = Vec::new();
    for pool in ch.active_pools() {
        let slots: Vec<String> = ch
            .all_spell_slots_for_pool(pool)
            .filter(|(_, slot)| slot.total > 0)
            .map(|(level, slot)| format!("L{level}: {}", slot.total))
            .collect();
        if !slots.is_empty() {
            parts.push(format!("{}: {}", i18n.tr(pool.tr_key()), slots.join(", ")));
        }
    }
    if parts.is_empty() {
        "\u{2014}".to_string()
    } else {
        parts.join(" | ")
    }
}

fn group_diff_rows(rows: Vec<DiffRow>) -> Vec<(&'static str, Vec<DiffRow>)> {
    let mut sections: Vec<(&'static str, Vec<DiffRow>)> = Vec::new();
    for row in rows {
        if sections.last().is_none_or(|(key, _)| *key != row.section) {
            sections.push((row.section, Vec::new()));
        }
        sections.last_mut().unwrap().1.push(row);
    }
    sections
}

impl Character {
    fn diff(&self, imported: &Character, i18n: leptos_fluent::I18n) -> Vec<DiffRow> {
        let mut rows = Vec::new();

        // --- Identity ---
        let sec = "diff-section-identity";
        push_if_diff(
            &mut rows,
            sec,
            "character-name",
            self.identity.name.clone(),
            imported.identity.name.clone(),
        );
        push_if_diff(
            &mut rows,
            sec,
            "species",
            self.identity.species.clone(),
            imported.identity.species.clone(),
        );
        push_if_diff(
            &mut rows,
            sec,
            "background",
            self.identity.background.clone(),
            imported.identity.background.clone(),
        );
        if self.identity.alignment != imported.identity.alignment {
            rows.push(DiffRow {
                section: sec,
                label: "alignment",
                local: i18n.tr(self.identity.alignment.tr_key()),
                imported: i18n.tr(imported.identity.alignment.tr_key()),
            });
        }
        push_if_diff(
            &mut rows,
            sec,
            "xp",
            self.identity.experience_points.to_string(),
            imported.identity.experience_points.to_string(),
        );
        let local_classes = self.class_summary();
        let imported_classes = imported.class_summary();
        if local_classes != imported_classes {
            rows.push(DiffRow {
                section: sec,
                label: "classes",
                local: local_classes,
                imported: imported_classes,
            });
        }

        // --- Ability Scores ---
        let sec = "panel-ability-scores";
        for ability in Ability::iter() {
            let local_score = self.ability_score(ability);
            let imported_score = imported.ability_score(ability);
            if local_score != imported_score {
                rows.push(DiffRow {
                    section: sec,
                    label: ability.tr_key(),
                    local: local_score.to_string(),
                    imported: imported_score.to_string(),
                });
            }
        }

        // --- Combat (skip death saves & temp HP — stripped during sharing) ---
        let sec = "panel-combat";
        push_if_diff(
            &mut rows,
            sec,
            "armor-class",
            self.armor_class().to_string(),
            imported.armor_class().to_string(),
        );
        push_if_diff(
            &mut rows,
            sec,
            "speed",
            self.speed().to_string(),
            imported.speed().to_string(),
        );
        push_if_diff(
            &mut rows,
            sec,
            "hp-max",
            self.hp_max().to_string(),
            imported.hp_max().to_string(),
        );
        push_if_diff(
            &mut rows,
            sec,
            "current-hp",
            self.hp_current().to_string(),
            imported.hp_current().to_string(),
        );
        push_if_diff(
            &mut rows,
            sec,
            "initiative",
            self.initiative().to_string(),
            imported.initiative().to_string(),
        );

        // --- Saving Throws ---
        let sec = "panel-saving-throws";
        for ability in Ability::iter() {
            let local_has = self.proficient_with(ability);
            let imported_has = imported.proficient_with(ability);
            if local_has != imported_has {
                rows.push(DiffRow {
                    section: sec,
                    label: ability.tr_abbr_key(),
                    local: (if local_has { "\u{25CF}" } else { "\u{25CB}" }).to_string(),
                    imported: (if imported_has { "\u{25CF}" } else { "\u{25CB}" }).to_string(),
                });
            }
        }

        // --- Skills ---
        let sec = "panel-skills";
        for skill in Skill::iter() {
            let local_level = self.skill_proficiency(skill);
            let imported_level = imported.skill_proficiency(skill);
            if local_level != imported_level {
                rows.push(DiffRow {
                    section: sec,
                    label: skill.tr_key(),
                    local: local_level.symbol().to_string(),
                    imported: imported_level.symbol().to_string(),
                });
            }
        }

        // --- Features (names only, descriptions stripped during sharing) ---
        let sec = "panel-features";
        let local_val = format_names(self.features(), |f| &f.name);
        let imported_val = format_names(imported.features(), |f| &f.name);
        push_if_diff(&mut rows, sec, "panel-features", local_val, imported_val);

        // --- Equipment ---
        let sec = "panel-equipment";
        let local_val = format_names(&self.equipment.weapons, |w| &w.name);
        let imported_val = format_names(&imported.equipment.weapons, |w| &w.name);
        push_if_diff(&mut rows, sec, "weapons", local_val, imported_val);

        let local_val = format_items(&self.equipment.items);
        let imported_val = format_items(&imported.equipment.items);
        push_if_diff(&mut rows, sec, "items", local_val, imported_val);

        if self.equipment.currency != imported.equipment.currency {
            rows.push(DiffRow {
                section: sec,
                label: "currency",
                local: self.equipment.currency.to_string(),
                imported: imported.equipment.currency.to_string(),
            });
        }

        // --- Spellcasting ---
        let sec = "panel-spellcasting";
        push_if_diff(
            &mut rows,
            sec,
            "spell-slots",
            format_spell_slots(self, i18n),
            format_spell_slots(imported, i18n),
        );
        {
            let all_keys: BTreeSet<&String> = self
                .feature_data
                .keys()
                .chain(imported.feature_data.keys())
                .collect();
            for key in all_keys {
                let local_sc = self.feature_data.get(key).and_then(|e| e.spells.as_ref());
                let imported_sc = imported
                    .feature_data
                    .get(key)
                    .and_then(|e| e.spells.as_ref());
                match (local_sc, imported_sc) {
                    (Some(local_sc), Some(imported_sc)) => {
                        if local_sc.casting_ability != imported_sc.casting_ability {
                            rows.push(DiffRow {
                                section: sec,
                                label: "casting-ability",
                                local: i18n.tr(local_sc.casting_ability.tr_key()),
                                imported: i18n.tr(imported_sc.casting_ability.tr_key()),
                            });
                        }
                        let local_val = format_names(&local_sc.spells, |spell| &spell.name);
                        let imported_val = format_names(&imported_sc.spells, |spell| &spell.name);
                        push_if_diff(&mut rows, sec, "spells", local_val, imported_val);
                    }
                    (Some(local_sc), None) => {
                        rows.push(DiffRow {
                            section: sec,
                            label: "enable-spellcasting",
                            local: i18n.tr(local_sc.casting_ability.tr_key()),
                            imported: "\u{2014}".to_string(),
                        });
                    }
                    (None, Some(imported_sc)) => {
                        rows.push(DiffRow {
                            section: sec,
                            label: "enable-spellcasting",
                            local: "\u{2014}".to_string(),
                            imported: i18n.tr(imported_sc.casting_ability.tr_key()),
                        });
                    }
                    (None, None) => {}
                }
            }
        }

        // --- Proficiencies & Languages ---
        let sec = "panel-proficiencies";
        for prof in Proficiency::iter() {
            let local_has = self.proficiencies.contains(&prof);
            let imported_has = imported.proficiencies.contains(&prof);
            if local_has != imported_has {
                rows.push(DiffRow {
                    section: sec,
                    label: prof.tr_key(),
                    local: (if local_has { "\u{25CF}" } else { "\u{25CB}" }).to_string(),
                    imported: (if imported_has { "\u{25CF}" } else { "\u{25CB}" }).to_string(),
                });
            }
        }
        let local_val = if self.languages.is_empty() {
            "\u{2014}".to_string()
        } else {
            self.languages.join(", ")
        };
        let imported_val = if imported.languages.is_empty() {
            "\u{2014}".to_string()
        } else {
            imported.languages.join(", ")
        };
        push_if_diff(&mut rows, sec, "languages", local_val, imported_val);

        // --- Personality ---
        let sec = "panel-personality";
        push_if_diff(
            &mut rows,
            sec,
            "history",
            truncate(&self.personality.history, 50),
            truncate(&imported.personality.history, 50),
        );
        push_if_diff(
            &mut rows,
            sec,
            "personality-traits",
            truncate(&self.personality.personality_traits, 50),
            truncate(&imported.personality.personality_traits, 50),
        );
        push_if_diff(
            &mut rows,
            sec,
            "ideals",
            truncate(&self.personality.ideals, 50),
            truncate(&imported.personality.ideals, 50),
        );
        push_if_diff(
            &mut rows,
            sec,
            "bonds",
            truncate(&self.personality.bonds, 50),
            truncate(&imported.personality.bonds, 50),
        );
        push_if_diff(
            &mut rows,
            sec,
            "flaws",
            truncate(&self.personality.flaws, 50),
            truncate(&imported.personality.flaws, 50),
        );

        // --- Notes ---
        let sec = "panel-notes";
        push_if_diff(
            &mut rows,
            sec,
            "panel-notes",
            truncate(&self.notes, 50),
            truncate(&imported.notes, 50),
        );

        rows
    }
}

// --- Restore stripped descriptions ---

fn restore_description_by_name<T>(
    imported: &mut [T],
    local: &[T],
    name_fn: fn(&T) -> &str,
    desc_fn: fn(&mut T) -> &mut String,
    get_desc: fn(&T) -> &str,
) {
    for item in imported.iter_mut() {
        if get_desc(item).is_empty()
            && let Some(local_item) = local.iter().find(|l| name_fn(l) == name_fn(item))
        {
            *desc_fn(item) = get_desc(local_item).to_string();
        }
    }
}

impl Character {
    pub fn restore_stripped_fields(&mut self, local: &Character) {
        // Restore temp combat state zeroed by strip_for_sharing
        self.combat.death_save_successes = local.combat.death_save_successes;
        self.combat.death_save_failures = local.combat.death_save_failures;
        self.combat.hp_temp = local.combat.hp_temp;

        // Restore descriptions stripped for sharing
        restore_description_by_name(
            &mut self.features,
            &local.features,
            |f| &f.name,
            |f| &mut f.description,
            |f| &f.description,
        );

        for (feature, entry) in &mut self.feature_data {
            let local_fields = local
                .feature_data
                .get(feature)
                .map(|e| e.fields.as_slice())
                .unwrap_or(&[]);

            for (field, local_field) in entry.fields.iter_mut().zip(local_fields.iter()) {
                field.description = local_field.description.clone();

                restore_description_by_name(
                    field.value.choices_mut(),
                    local_field.value.choices(),
                    |c| &c.name,
                    |c| &mut c.description,
                    |c| &c.description,
                );
            }

            if let (Some(imp_sc), Some(loc_entry)) =
                (&mut entry.spells, local.feature_data.get(feature))
                && let Some(loc_sc) = &loc_entry.spells
            {
                restore_description_by_name(
                    &mut imp_sc.spells,
                    &loc_sc.spells,
                    |s| &s.name,
                    |s| &mut s.description,
                    |s| &s.description,
                );
            }
        }
    }
}

fn do_import(mut character: Character) -> impl IntoView {
    if let Some(existing) = storage::load_character(&character.id) {
        character.restore_stripped_fields(&existing);
    }
    storage::save_and_sync_character(&mut character);
    let id = character.id;

    let navigate = use_navigate();
    request_animation_frame(move || {
        navigate(&format!("/c/{id}"), Default::default());
    });

    view! { <p>"Importing..."</p> }
}

#[component]
pub fn ImportConflict(incoming: Character, existing: Character) -> impl IntoView {
    let incoming = StoredValue::new(incoming);
    let existing = StoredValue::new(existing);
    let i18n = expect_context::<leptos_fluent::I18n>();

    let save_character = move |character: &mut Character| {
        character.restore_stripped_fields(&existing.read_value());
        storage::save_and_sync_character(character);
        let navigate = use_navigate();
        navigate(&format!("/c/{}", character.id), Default::default());
    };

    let import_anyway = move |_| {
        let mut character = incoming.write_value();
        save_character(&mut character);
    };

    let import_as_copy = move |_| {
        let mut character = incoming.write_value();
        character.id = Uuid::new_v4();
        character.identity.name = format!("{} (Copy)", character.identity.name);
        save_character(&mut character);
    };

    let name = existing.read_value().identity.name.clone();
    let message = move_tr!("import-conflict-message", { "name" => name.clone() });

    let diff_rows = untrack(|| existing.read_value().diff(&incoming.read_value(), i18n));
    let sections = group_diff_rows(diff_rows);
    let has_diffs = !sections.is_empty();

    view! {
        <div class="import-conflict panel">
            <h2>{move_tr!("import-conflict-title")}</h2>
            <p>{message}</p>

            {if has_diffs {
                Either::Left(view! {
                    <table class="diff-table">
                        <thead>
                            <tr>
                                <th>{move_tr!("diff-field")}</th>
                                <th class="diff-local">{move_tr!("diff-local")}</th>
                                <th class="diff-imported">{move_tr!("diff-imported")}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {sections
                                .into_iter()
                                .map(|(section_key, rows)| {
                                    let section_title = untrack(|| i18n.tr(section_key));
                                    view! {
                                        <tr class="diff-section">
                                            <td colspan="3">{section_title}</td>
                                        </tr>
                                        {rows
                                            .into_iter()
                                            .map(|row| {
                                                let label = untrack(|| i18n.tr(row.label));
                                                view! {
                                                    <tr>
                                                        <td>{label}</td>
                                                        <td class="diff-local">{row.local}</td>
                                                        <td class="diff-imported">
                                                            {row.imported}
                                                        </td>
                                                    </tr>
                                                }
                                            })
                                            .collect_view()}
                                    }
                                })
                                .collect_view()}
                        </tbody>
                    </table>
                })
            } else {
                Either::Right(view! {
                    <p class="diff-no-differences">{move_tr!("diff-no-differences")}</p>
                })
            }}

            <div class="import-conflict-actions">
                <button class="btn-primary" on:click=import_anyway>{move_tr!("import-anyway")}</button>
                <button class="btn-primary" on:click=import_as_copy>{move_tr!("import-as-copy")}</button>
                <A href=format!("{BASE_URL}/") attr:class="btn-cancel">{move_tr!("import-cancel")}</A>
            </div>
        </div>
    }
}

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct ImportParams {
    data: String,
}

#[component]
pub fn ImportCharacter() -> impl IntoView {
    let data = use_params::<ImportParams>()
        .get_untracked()
        .ok()
        .map(|p| p.data);

    let error_view = move || {
        view! {
            <div class="panel">
                <h2>{move_tr!("share-error")}</h2>
                <A href=format!("{BASE_URL}/")>{move_tr!("back-to-list")}</A>
            </div>
        }
    };

    let Some(data) = data else {
        return Either::Left(error_view());
    };

    let character = LocalResource::new(move || {
        let data = data.clone();
        async move { share::decode_character(&data).await }
    });

    Either::Right(view! {
        <Suspense fallback=move || view! {
            <div class="panel">
                <p>{move_tr!("share-loading")}</p>
            </div>
        }>
            {move || {
                character.get().map(|result| {
                    match result {
                        Some(ch) => Either::Left(import_or_conflict(ch)),
                        None => Either::Right(error_view()),
                    }
                })
            }}
        </Suspense>
    })
}

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct CloudImportParams {
    user_id: String,
    char_id: String,
}

pub fn import_or_conflict(character: Character) -> impl IntoView {
    let existing = storage::load_character(&character.id);
    let has_conflict = existing
        .as_ref()
        .is_some_and(|existing| existing.updated_at > character.updated_at);

    if has_conflict {
        Either::Left(view! {
            <ImportConflict incoming=character existing=existing.unwrap() />
        })
    } else {
        Either::Right(do_import(character))
    }
}

#[component]
pub fn ImportCloudCharacter() -> impl IntoView {
    let params = use_params::<CloudImportParams>().get_untracked().ok();

    let not_found_view = move || {
        view! {
            <div class="panel">
                <h2>{move_tr!("share-not-found")}</h2>
                <A href=format!("{BASE_URL}/")>{move_tr!("back-to-list")}</A>
            </div>
        }
    };

    let Some(params) = params else {
        return Either::Left(not_found_view());
    };

    let user_id = params.user_id;
    let char_id = params.char_id;

    let character = LocalResource::new(move || {
        let uid = user_id.clone();
        let cid = char_id.clone();
        async move {
            firebase::wait_ready().await;
            let value =
                firebase::get_doc::<serde_json::Value>(&["users", &uid, "characters", &cid])
                    .await
                    .ok()??;
            let ch = storage::deserialize_character_value(value)?;
            ch.shared.then_some(ch)
        }
    });

    Either::Right(view! {
        <Suspense fallback=move || view! {
            <div class="panel">
                <p>{move_tr!("share-loading")}</p>
            </div>
        }>
            {move || {
                character.get().map(|result| {
                    match result {
                        Some(ch) => Either::Left(import_or_conflict(ch)),
                        None => Either::Right(not_found_view()),
                    }
                })
            }}
        </Suspense>
    })
}

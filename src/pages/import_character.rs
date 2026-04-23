use std::collections::BTreeSet;

use js_sys::Date;
use leptos::{either::Either, prelude::*};
use leptos_fluent::{move_tr, tr};
use leptos_router::{
    hooks::{use_navigate, use_params},
    params::Params,
};
use strum::IntoEnumIterator;
use uuid::Uuid;
use wasm_bindgen::JsValue;

use crate::{
    components::{cloud_sign_in_hint::CloudSignInHint, ref_link::Ref},
    firebase,
    model::{Ability, Avatar, Character, Item, Note, Proficiency, Skill, Translatable},
    storage,
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

/// Compact one-line summary of a notes vector for import diff: shows the
/// last note with its level/date plus the total count. Empty vector returns
/// an empty string so equal-empty states don't trigger a diff row.
fn notes_diff_summary(notes: &[Note], i18n: leptos_fluent::I18n) -> String {
    let Some(last) = notes.last() else {
        return String::new();
    };
    let level_label = if last.level > 0 {
        tr!("slot-level", {"level" => last.level})
    } else {
        "\u{2014}".into()
    };
    let date_label: String = if last.created_at > 0 {
        let date = Date::new(&((last.created_at as f64) * 1000.0).into());
        date.to_locale_date_string(i18n.language.get().id, &JsValue::UNDEFINED)
            .into()
    } else {
        "\u{2014}".into()
    };
    tr!("diff-notes-summary", {
        "text" => truncate(&last.text, 50),
        "level" => level_label,
        "date" => date_label,
        "count" => notes.len(),
    })
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
    for pool in ch.spell_slots.active_pools() {
        let slots: Vec<String> = ch
            .spell_slots
            .iter_pool(pool)
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
        if self.personality.alignment != imported.personality.alignment {
            rows.push(DiffRow {
                section: sec,
                label: "alignment",
                local: i18n.tr(self.personality.alignment.tr_key()),
                imported: i18n.tr(imported.personality.alignment.tr_key()),
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
            let local_level = self.skills.get(skill);
            let imported_level = imported.skills.get(skill);
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
                .features
                .keys()
                .chain(imported.features.keys())
                .collect();
            for key in all_keys {
                let local_sc = self.features.get(key).and_then(|e| e.spells.as_ref());
                let imported_sc = imported.features.get(key).and_then(|e| e.spells.as_ref());
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
            notes_diff_summary(&self.notes, i18n),
            notes_diff_summary(&imported.notes, i18n),
        );

        rows
    }
}

fn do_import(mut character: Character, avatar: Option<Avatar>) -> impl IntoView {
    storage::save_and_sync_character(&mut character);
    if let Some(avatar) = avatar {
        storage::save_avatar(&character.id, &avatar);
    }
    let id = character.id;

    let navigate = use_navigate();
    request_animation_frame(move || {
        navigate(&format!("/c/{id}"), Default::default());
    });

    view! { <p>"Importing..."</p> }
}

#[component]
pub fn ImportConflict(
    incoming: Character,
    existing: Character,
    avatar: Option<Avatar>,
) -> impl IntoView {
    let incoming = StoredValue::new(incoming);
    let existing = StoredValue::new(existing);
    let avatar = StoredValue::new(avatar);
    let i18n = expect_context::<leptos_fluent::I18n>();

    let save_character = move |character: &mut Character, id: Uuid| {
        storage::save_and_sync_character(character);
        if let Some(av) = &*avatar.read_value() {
            storage::save_avatar(&id, av);
        }
        let navigate = use_navigate();
        navigate(&format!("/c/{}", character.id), Default::default());
    };

    let import_anyway = move |_| {
        let mut character = incoming.write_value();
        let id = character.id;
        save_character(&mut character, id);
    };

    let import_as_copy = move |_| {
        let mut character = incoming.write_value();
        let new_id = Uuid::new_v4();
        character.id = new_id;
        character.identity.name = format!("{} (Copy)", character.identity.name);
        save_character(&mut character, new_id);
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
                <Ref href="/" attr:class="btn-cancel">{move_tr!("import-cancel")}</Ref>
            </div>
        </div>
    }
}

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct CloudImportParams {
    user_id: String,
    char_id: String,
}

pub fn import_or_conflict(character: Character, avatar: Option<Avatar>) -> impl IntoView {
    let existing = storage::load_character(&character.id);
    let has_conflict = existing
        .as_ref()
        .is_some_and(|existing| existing.updated_at > character.updated_at);

    if has_conflict {
        Either::Left(view! {
            <ImportConflict incoming=character existing=existing.unwrap() avatar=avatar />
        })
    } else {
        Either::Right(do_import(character, avatar))
    }
}

#[component]
pub fn ImportCloudCharacter() -> impl IntoView {
    let params = use_params::<CloudImportParams>().get_untracked().ok();

    let not_found_view = move || {
        view! {
            <div class="panel">
                <h2>{move_tr!("share-not-found")}</h2>
                <CloudSignInHint message=move_tr!("hint-character-not-found") />
                <Ref href="/">{move_tr!("back-to-list")}</Ref>
            </div>
        }
    };

    let Some(params) = params else {
        return Either::Left(not_found_view());
    };

    let user_id = params.user_id;
    let char_id = params.char_id;

    let character = LocalResource::new(move || {
        let user_id = user_id.clone();
        let char_id = char_id.clone();
        async move {
            firebase::wait_ready().await;
            let value = firebase::get_doc::<serde_json::Value>(&[
                "users",
                &user_id,
                "characters",
                &char_id,
            ])
            .await
            .ok()??;
            let character = storage::deserialize_character_value(value)?;
            let character = character.shared.then_some(character)?;
            let avatar = if let Ok(char_uuid) = char_id.parse::<Uuid>() {
                storage::get_avatar_doc(&user_id, char_uuid).await
            } else {
                None
            };
            Some((character, avatar))
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
                        Some((character, avatar)) => Either::Left(import_or_conflict(character, avatar)),
                        None => Either::Right(not_found_view()),
                    }
                })
            }}
        </Suspense>
    })
}

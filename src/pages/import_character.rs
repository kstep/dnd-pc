use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use leptos_router::{
    components::A,
    hooks::{use_navigate, use_params},
    params::Params,
};
use strum::IntoEnumIterator;

use crate::{
    BASE_URL,
    model::{
        Ability, Character, Proficiency, ProficiencyLevel, Skill, SpellcastingData, Translatable,
    },
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

fn format_spell_slots(sc: &SpellcastingData) -> String {
    let slots: Vec<String> = sc
        .all_spell_slots()
        .filter(|(_, slot)| slot.total > 0)
        .map(|(level, slot)| format!("L{level}: {}", slot.total))
        .collect();
    if slots.is_empty() {
        "\u{2014}".to_string()
    } else {
        slots.join(", ")
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

fn compute_diff(
    local: &Character,
    imported: &Character,
    i18n: leptos_fluent::I18n,
) -> Vec<DiffRow> {
    let mut rows = Vec::new();

    // --- Identity ---
    let sec = "diff-section-identity";
    push_if_diff(
        &mut rows,
        sec,
        "character-name",
        local.identity.name.clone(),
        imported.identity.name.clone(),
    );
    push_if_diff(
        &mut rows,
        sec,
        "race",
        local.identity.race.clone(),
        imported.identity.race.clone(),
    );
    push_if_diff(
        &mut rows,
        sec,
        "background",
        local.identity.background.clone(),
        imported.identity.background.clone(),
    );
    if local.identity.alignment != imported.identity.alignment {
        rows.push(DiffRow {
            section: sec,
            label: "alignment",
            local: i18n.tr(local.identity.alignment.tr_key()),
            imported: i18n.tr(imported.identity.alignment.tr_key()),
        });
    }
    push_if_diff(
        &mut rows,
        sec,
        "xp",
        local.identity.experience_points.to_string(),
        imported.identity.experience_points.to_string(),
    );
    if local.class_summary() != imported.class_summary() {
        rows.push(DiffRow {
            section: sec,
            label: "classes",
            local: local.class_summary(),
            imported: imported.class_summary(),
        });
    }

    // --- Ability Scores ---
    let sec = "panel-ability-scores";
    for ability in Ability::iter() {
        let local_score = local.abilities.get(ability);
        let imported_score = imported.abilities.get(ability);
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
        local.combat.armor_class.to_string(),
        imported.combat.armor_class.to_string(),
    );
    push_if_diff(
        &mut rows,
        sec,
        "speed",
        local.combat.speed.to_string(),
        imported.combat.speed.to_string(),
    );
    push_if_diff(
        &mut rows,
        sec,
        "hp-max",
        local.combat.hp_max.to_string(),
        imported.combat.hp_max.to_string(),
    );
    push_if_diff(
        &mut rows,
        sec,
        "current-hp",
        local.combat.hp_current.to_string(),
        imported.combat.hp_current.to_string(),
    );
    push_if_diff(
        &mut rows,
        sec,
        "initiative",
        local.combat.initiative_misc_bonus.to_string(),
        imported.combat.initiative_misc_bonus.to_string(),
    );

    // --- Saving Throws ---
    let sec = "panel-saving-throws";
    for ability in Ability::iter() {
        let local_has = local.saving_throws.contains(&ability);
        let imported_has = imported.saving_throws.contains(&ability);
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
        let local_level = local
            .skills
            .get(&skill)
            .copied()
            .unwrap_or(ProficiencyLevel::None);
        let imported_level = imported
            .skills
            .get(&skill)
            .copied()
            .unwrap_or(ProficiencyLevel::None);
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
    let local_val = format_names(&local.features, |f| &f.name);
    let imported_val = format_names(&imported.features, |f| &f.name);
    push_if_diff(&mut rows, sec, "panel-features", local_val, imported_val);

    // --- Equipment ---
    let sec = "panel-equipment";
    let local_val = format_names(&local.equipment.weapons, |w| &w.name);
    let imported_val = format_names(&imported.equipment.weapons, |w| &w.name);
    push_if_diff(&mut rows, sec, "weapons", local_val, imported_val);

    let local_val = format_names(&local.equipment.items, |item| &item.name);
    let imported_val = format_names(&imported.equipment.items, |item| &item.name);
    push_if_diff(&mut rows, sec, "items", local_val, imported_val);

    if local.equipment.currency != imported.equipment.currency {
        rows.push(DiffRow {
            section: sec,
            label: "currency",
            local: local.equipment.currency.to_string(),
            imported: imported.equipment.currency.to_string(),
        });
    }

    // --- Spellcasting ---
    let sec = "panel-spellcasting";
    match (&local.spellcasting, &imported.spellcasting) {
        (None, None) => {}
        (Some(local_sc), Some(imported_sc)) => {
            if local_sc.casting_ability != imported_sc.casting_ability {
                rows.push(DiffRow {
                    section: sec,
                    label: "casting-ability",
                    local: i18n.tr(local_sc.casting_ability.tr_key()),
                    imported: i18n.tr(imported_sc.casting_ability.tr_key()),
                });
            }
            push_if_diff(
                &mut rows,
                sec,
                "spell-slots",
                format_spell_slots(local_sc),
                format_spell_slots(imported_sc),
            );
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
    }

    // --- Proficiencies & Languages ---
    let sec = "panel-proficiencies";
    for prof in Proficiency::iter() {
        let local_has = local.proficiencies.contains(&prof);
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
    let local_val = if local.languages.is_empty() {
        "\u{2014}".to_string()
    } else {
        local.languages.join(", ")
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
        truncate(&local.personality.history, 50),
        truncate(&imported.personality.history, 50),
    );
    push_if_diff(
        &mut rows,
        sec,
        "personality-traits",
        truncate(&local.personality.personality_traits, 50),
        truncate(&imported.personality.personality_traits, 50),
    );
    push_if_diff(
        &mut rows,
        sec,
        "ideals",
        truncate(&local.personality.ideals, 50),
        truncate(&imported.personality.ideals, 50),
    );
    push_if_diff(
        &mut rows,
        sec,
        "bonds",
        truncate(&local.personality.bonds, 50),
        truncate(&imported.personality.bonds, 50),
    );
    push_if_diff(
        &mut rows,
        sec,
        "flaws",
        truncate(&local.personality.flaws, 50),
        truncate(&imported.personality.flaws, 50),
    );

    // --- Racial Traits (names only, descriptions stripped during sharing) ---
    let sec = "racial-traits";
    let local_val = format_names(&local.racial_traits, |t| &t.name);
    let imported_val = format_names(&imported.racial_traits, |t| &t.name);
    push_if_diff(&mut rows, sec, "racial-traits", local_val, imported_val);

    // --- Notes ---
    let sec = "panel-notes";
    push_if_diff(
        &mut rows,
        sec,
        "panel-notes",
        truncate(&local.notes, 50),
        truncate(&imported.notes, 50),
    );

    rows
}

// --- Components ---

fn do_import(character: &Character) -> impl IntoView {
    storage::save_character(character);
    let id = character.id;

    let navigate = use_navigate();
    request_animation_frame(move || {
        navigate(&format!("{BASE_URL}/c/{id}"), Default::default());
    });

    view! { <p>"Importing..."</p> }
}

#[component]
fn ImportConflict(incoming: Character, existing: Character) -> impl IntoView {
    let id = incoming.id;
    let incoming = StoredValue::new(incoming);
    let existing = StoredValue::new(existing);
    let i18n = expect_context::<leptos_fluent::I18n>();

    let import_anyway = move |_| {
        storage::save_character(&incoming.get_value());
        let navigate = use_navigate();
        navigate(&format!("{BASE_URL}/c/{id}"), Default::default());
    };

    let name = existing.get_value().identity.name.clone();
    let message = tr!("import-conflict-message", { "name" => name });

    let diff_rows = compute_diff(&existing.get_value(), &incoming.get_value(), i18n);
    let sections = group_diff_rows(diff_rows);
    let has_diffs = !sections.is_empty();

    view! {
        <div class="panel">
            <h2>{move_tr!("import-conflict-title")}</h2>
            <p>{message}</p>

            {if has_diffs {
                view! {
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
                                    let section_title = i18n.tr(section_key);
                                    view! {
                                        <tr class="diff-section">
                                            <td colspan="3">{section_title}</td>
                                        </tr>
                                        {rows
                                            .into_iter()
                                            .map(|row| {
                                                let label = i18n.tr(row.label);
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
                }
                    .into_any()
            } else {
                view! {
                    <p class="diff-no-differences">{move_tr!("diff-no-differences")}</p>
                }
                    .into_any()
            }}

            <div class="import-conflict-actions">
                <button on:click=import_anyway>{move_tr!("import-anyway")}</button>
                <A href=format!("{BASE_URL}/")>{move_tr!("import-cancel")}</A>
            </div>
        </div>
    }
}

#[derive(Params, Clone, Debug, PartialEq)]
struct ImportParams {
    data: String,
}

#[component]
pub fn ImportCharacter() -> impl IntoView {
    let data = use_params::<ImportParams>().get().ok().map(|p| p.data);

    match data {
        Some(data) => match share::decode_character(&data) {
            Some(character) => {
                let existing = storage::load_character(&character.id);
                let has_conflict = existing
                    .as_ref()
                    .is_some_and(|existing| existing.updated_at > character.updated_at);

                if has_conflict {
                    let existing = existing.unwrap();
                    view! {
                        <ImportConflict incoming=character existing=existing />
                    }
                    .into_any()
                } else {
                    do_import(&character).into_any()
                }
            }
            None => view! {
                <div class="panel">
                    <h2>{move_tr!("share-error")}</h2>
                    <A href=format!("{BASE_URL}/")>{move_tr!("back-to-list")}</A>
                </div>
            }
            .into_any(),
        },
        None => view! {
            <div class="panel">
                <h2>{move_tr!("share-error")}</h2>
                <A href=format!("{BASE_URL}/")>{move_tr!("back-to-list")}</A>
            </div>
        }
        .into_any(),
    }
}

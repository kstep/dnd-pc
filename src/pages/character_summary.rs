use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{summary_header::SummaryHeader, toggle_button::ToggleButton},
    model::{
        Ability, Character, CharacterIdentityStoreFields, CharacterStoreFields,
        CombatStatsStoreFields, EquipmentStoreFields, FeatureValue, ProficiencyLevel, Skill,
        Translatable, format_bonus,
    },
    rules::RulesRegistry,
};

#[component]
pub fn CharacterSummary() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let registry = expect_context::<RulesRegistry>();

    let combat = store.combat();

    // --- Core computed values with targeted tracking ---
    let prof_bonus = Memo::new(move |_| {
        let level = store
            .identity()
            .classes()
            .read()
            .iter()
            .map(|class| class.level)
            .sum::<u32>()
            .max(1);
        (level as i32 - 1) / 4 + 2
    });

    let initiative = Memo::new(move |_| {
        let dex_score = store.abilities().get().dexterity as i32;
        let dex_mod = (dex_score - 10).div_euclid(2);
        dex_mod + combat.initiative_misc_bonus().get()
    });

    view! {
        <SummaryHeader />
        <div class="summary-page">

            // === Section: What Can I Do? ===
            <div class="summary-section">
                <h3 class="summary-section-title">{move_tr!("summary-actions")}</h3>

                // -- Weapons (tracks only equipment.weapons) --
                {move || {
                    let weapons = store.equipment().weapons().read();
                    let rows: Vec<_> = weapons.iter().filter(|w| !w.name.is_empty()).map(|w| {
                        let name = w.name.clone();
                        let atk = w.attack_bonus.clone();
                        let dmg = w.damage.clone();
                        let dmg_type = w.damage_type.map(|dt| {
                            i18n.tr(dt.tr_key()).to_string()
                        }).unwrap_or_default();
                        (name, atk, dmg, dmg_type)
                    }).collect();
                    if rows.is_empty() {
                        view! {
                            <p class="summary-empty">{move_tr!("summary-no-weapons")}</p>
                        }.into_any()
                    } else {
                        view! {
                            <table class="summary-table">
                                <thead>
                                    <tr>
                                        <th>{move_tr!("name")}</th>
                                        <th>{move_tr!("atk-bonus")}</th>
                                        <th>{move_tr!("damage")}</th>
                                        <th>{move_tr!("type")}</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {rows.into_iter().map(|(name, atk, dmg, dtype)| {
                                        view! {
                                            <tr>
                                                <td>{name}</td>
                                                <td>{atk}</td>
                                                <td>{dmg}</td>
                                                <td>{dtype}</td>
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        }.into_any()
                    }
                }}

                // -- Spells (tracks feature_data, spell_slots, abilities, identity.classes) --
                {move || {
                    let feature_data = store.feature_data().read();
                    let abilities = store.abilities().get();
                    let prof = prof_bonus.get();
                    let identity = store.identity().read();

                    let spell_sections: Vec<_> = feature_data.iter()
                        .filter_map(|(name, entry)| {
                            let spell_data = entry.spells.as_ref()?;
                            let ability = spell_data.casting_ability;
                            let ability_mod = (abilities.get(ability) as i32 - 10).div_euclid(2);
                            let save_dc = 8 + prof + ability_mod;
                            let atk_bonus = prof + ability_mod;

                            let feature_label = registry
                                .with_feature(&identity, name, |f| f.label().to_string())
                                .unwrap_or_else(|| name.clone());

                            let spell_slots = store.spell_slots().read();
                            let all_spells: Vec<_> = spell_data.spells.iter()
                                .filter(|spell| {
                                    if spell.name.is_empty() {
                                        return false;
                                    }
                                    // Cantrips: always show prepared/sticky
                                    if spell.level == 0 {
                                        return spell.prepared || spell.sticky;
                                    }
                                    // Leveled: must be prepared/sticky and have available slots
                                    if !spell.prepared && !spell.sticky {
                                        return false;
                                    }
                                    (spell.level..=9).any(|sl| {
                                        let idx = (sl - 1) as usize;
                                        spell_slots.get(idx).is_some_and(|slot| {
                                            slot.total > 0 && slot.used < slot.total
                                        })
                                    })
                                })
                                .map(|spell| (spell.label().to_string(), spell.level, spell.description.clone()))
                                .collect();

                            if all_spells.is_empty() {
                                return None;
                            }

                            Some((feature_label, save_dc, atk_bonus, all_spells))
                        })
                        .collect();

                    if spell_sections.is_empty() {
                        return ().into_any();
                    }

                    spell_sections.into_iter().map(|(label, dc, atk, spells)| {
                        let atk_str = format_bonus(atk);
                        let expanded = RwSignal::new(HashSet::<usize>::new());
                        view! {
                            <div class="summary-spell-section">
                                <h4>{label}</h4>
                                <div class="summary-spell-stats">
                                    <span class="summary-spell-stat">
                                        {move_tr!("spell-save-dc")} ": " <strong>{dc}</strong>
                                    </span>
                                    <span class="summary-spell-stat">
                                        {move_tr!("spell-attack")} ": " <strong>{atk_str}</strong>
                                    </span>
                                </div>
                                <div class="summary-spells-list">
                                    {spells.into_iter().enumerate().map(|(i, (name, level, desc))| {
                                        let is_open = Signal::derive(move || expanded.get().contains(&i));
                                        let has_desc = !desc.is_empty();
                                        let level_str = if level == 0 {
                                            tr!("summary-cantrips").to_string()
                                        } else {
                                            tr!("slot-level", {"level" => level.to_string()}).to_string()
                                        };
                                        view! {
                                            <div class="summary-spell-entry">
                                                <div class="summary-spell-row">
                                                    {if has_desc {
                                                        Some(view! {
                                                            <ToggleButton
                                                                expanded=is_open
                                                                on_toggle=move || expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                                            />
                                                        })
                                                    } else {
                                                        None
                                                    }}
                                                    <span class="summary-spell-name">{name}</span>
                                                    <span class="summary-spell-level-label">{level_str}</span>
                                                </div>
                                                <Show when=move || is_open.get() && has_desc>
                                                    <p class="summary-spell-desc">{desc.clone()}</p>
                                                </Show>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>
                        }
                    }).collect_view().into_any()
                }}

                // -- Feature resources (tracks only feature_data) --
                {move || {
                    let feature_data = store.feature_data().read();
                    // Collect (feat_name, field_index, label, kind) for each resource
                    enum ResourceKind {
                        Points { used: u32, max: u32 },
                        Die(String),
                    }
                    let resources: Vec<_> = feature_data.iter()
                        .flat_map(|(feat_name, entry)| {
                            entry.fields.iter().enumerate().filter_map(|(field_idx, field)| {
                                match &field.value {
                                    FeatureValue::Points { used, max } if *max > 0 => {
                                        Some((feat_name.clone(), field_idx, field.label().to_string(), ResourceKind::Points { used: *used, max: *max }))
                                    }
                                    FeatureValue::Die(val) if !val.is_empty() => {
                                        Some((feat_name.clone(), field_idx, field.label().to_string(), ResourceKind::Die(val.clone())))
                                    }
                                    _ => None,
                                }
                            })
                        })
                        .collect();

                    if resources.is_empty() {
                        return ().into_any();
                    }

                    view! {
                        <h4 class="summary-subsection-title">{move_tr!("summary-resources")}</h4>
                        <div class="summary-spell-slots">
                            {resources.into_iter().map(|(feat_name, field_idx, label, kind)| {
                                match kind {
                                    ResourceKind::Points { used, max } => {
                                        view! {
                                            <div class="summary-slot">
                                                <span class="summary-slot-level">{label}</span>
                                                <input
                                                    type="number"
                                                    class="short-input"
                                                    min="0"
                                                    prop:max=max.to_string()
                                                    prop:value=used.to_string()
                                                    on:input={
                                                        let feat_name = feat_name.clone();
                                                        move |event| {
                                                            if let Ok(value) = event_target_value(&event).parse::<u32>() {
                                                                store.feature_data().update(|map| {
                                                                    if let Some(entry) = map.get_mut(&feat_name)
                                                                        && let Some(field) = entry.fields.get_mut(field_idx)
                                                                        && let FeatureValue::Points { used, .. } = &mut field.value
                                                                    {
                                                                        *used = value;
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    }
                                                />
                                                <span>"/" {max}</span>
                                            </div>
                                        }.into_any()
                                    }
                                    ResourceKind::Die(val) => {
                                        view! {
                                            <div class="summary-slot">
                                                <span class="summary-slot-level">{label}</span>
                                                <span>{val}</span>
                                            </div>
                                        }.into_any()
                                    }
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}

                // -- Choice fields (metamagic, known items, etc.) --
                {move || {
                    let feature_data = store.feature_data().read();
                    let choices: Vec<_> = feature_data.iter()
                        .flat_map(|(_feat_name, entry)| {
                            entry.fields.iter().filter_map(|field| {
                                if let FeatureValue::Choice { options } = &field.value {
                                    let selected: Vec<_> = options.iter()
                                        .map(|opt| (opt.label().to_string(), opt.description.clone()))
                                        .collect();
                                    if selected.is_empty() {
                                        return None;
                                    }
                                    Some((field.label().to_string(), selected))
                                } else {
                                    None
                                }
                            })
                        })
                        .collect();

                    if choices.is_empty() {
                        return ().into_any();
                    }

                    choices.into_iter().map(|(label, options)| {
                        let expanded = RwSignal::new(HashSet::<usize>::new());
                        view! {
                            <h4 class="summary-subsection-title">{label}</h4>
                            <div class="summary-spells-list">
                                {options.into_iter().enumerate().map(|(i, (name, desc))| {
                                    let is_open = Signal::derive(move || expanded.get().contains(&i));
                                    let has_desc = !desc.is_empty();
                                    view! {
                                        <div class="summary-spell-entry">
                                            <div class="summary-spell-row">
                                                {if has_desc {
                                                    Some(view! {
                                                        <ToggleButton
                                                            expanded=is_open
                                                            on_toggle=move || expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                                        />
                                                    })
                                                } else {
                                                    None
                                                }}
                                                <span class="summary-spell-name">{name}</span>
                                            </div>
                                            <Show when=move || is_open.get() && has_desc>
                                                <p class="summary-spell-desc">{desc.clone()}</p>
                                            </Show>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        }
                    }).collect_view().into_any()
                }}
            </div>

            // === Section: Main Stats ===
            <div class="summary-section">
                <h3 class="summary-section-title">{move_tr!("summary-stats")}</h3>

                // -- HP: [current] / {max+temp} ({max} + [temp]) --
                <div class="summary-core-stats">
                    <div class="summary-stat-box summary-hp-box">
                        <label>{move_tr!("hp")}</label>
                        <div class="summary-hp-value">
                            <input
                                type="number"
                                class="summary-stat-input"
                                prop:value=move || combat.hp_current().get().to_string()
                                on:input=move |e| {
                                    if let Ok(value) = event_target_value(&e).parse::<i32>() {
                                        combat.hp_current().set(value);
                                    }
                                }
                            />
                            <span class="summary-hp-max">
                                "/ " {move || combat.hp_max().get() + combat.hp_temp().get()}
                            </span>
                            <span class="summary-hp-detail">
                                "(" {move || combat.hp_max().get()} " + "
                                <input
                                    type="number"
                                    class="summary-hp-temp-input"
                                    prop:value=move || combat.hp_temp().get().to_string()
                                    on:input=move |e| {
                                        if let Ok(value) = event_target_value(&e).parse::<i32>() {
                                            combat.hp_temp().set(value);
                                        }
                                    }
                                />
                                ")"
                            </span>
                        </div>
                    </div>
                </div>

                // -- Death saves (shown when HP <= 0) --
                <Show when=move || combat.hp_current().get() <= 0>
                    <div class="summary-death-saves">
                        <div class="death-save-row">
                            <span>{move_tr!("successes")}</span>
                            <div class="death-save-boxes">
                                {(0u8..3)
                                    .map(|i| {
                                        let checked = move || combat.death_save_successes().get() > i;
                                        view! {
                                            <button
                                                class="death-save-box"
                                                class:filled=checked
                                                on:click=move |_| {
                                                    let current = combat.death_save_successes().get();
                                                    if current > i {
                                                        combat.death_save_successes().set(i);
                                                    } else {
                                                        combat.death_save_successes().set(i + 1);
                                                    }
                                                }
                                            >
                                                {move || if checked() { "\u{25CF}" } else { "\u{25CB}" }}
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                        <div class="death-save-row">
                            <span>{move_tr!("failures")}</span>
                            <div class="death-save-boxes">
                                {(0u8..3)
                                    .map(|i| {
                                        let checked = move || combat.death_save_failures().get() > i;
                                        view! {
                                            <button
                                                class="death-save-box"
                                                class:filled=checked
                                                on:click=move |_| {
                                                    let current = combat.death_save_failures().get();
                                                    if current > i {
                                                        combat.death_save_failures().set(i);
                                                    } else {
                                                        combat.death_save_failures().set(i + 1);
                                                    }
                                                }
                                            >
                                                {move || if checked() { "\u{25CF}" } else { "\u{25CB}" }}
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                    </div>
                </Show>

                // -- Core stats: AC, Initiative, Speed --
                <div class="summary-core-stats">
                    <div class="summary-stat-box">
                        <label>{move_tr!("armor-class")}</label>
                        <span>{move || combat.armor_class().get()}</span>
                    </div>
                    <div class="summary-stat-box">
                        <label>{move_tr!("initiative")}</label>
                        <span>{move || format_bonus(initiative.get())}</span>
                    </div>
                    <div class="summary-stat-box">
                        <label>{move_tr!("speed")}</label>
                        <span>{move || combat.speed().get()}</span>
                    </div>
                </div>

                // -- Ability modifiers (tracks only abilities) --
                <h4 class="summary-subsection-title">{move_tr!("summary-ability-mods")}</h4>
                <div class="summary-abilities-grid">
                    {Ability::iter().map(|ability| {
                        let tr_key = ability.tr_abbr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        view! {
                            <div class="summary-ability">
                                <span class="summary-ability-label">{label}</span>
                                <span class="summary-ability-mod">{move || {
                                    let score = store.abilities().get().get(ability) as i32;
                                    format_bonus((score - 10).div_euclid(2))
                                }}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>

                // -- Saving throws (tracks abilities, saving_throws, classes) --
                <h4 class="summary-subsection-title">{move_tr!("summary-saving-throws")}</h4>
                <div class="summary-saves-grid">
                    {Ability::iter().map(|ability| {
                        let tr_key = ability.tr_abbr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        let proficient = move || store.saving_throws().read().contains(&ability);
                        view! {
                            <div class="summary-save" class:proficient=proficient>
                                <span class="summary-save-label">{label}</span>
                                <span class="summary-save-value">{move || {
                                    let score = store.abilities().get().get(ability) as i32;
                                    let modifier = (score - 10).div_euclid(2);
                                    let bonus = modifier + if store.saving_throws().read().contains(&ability) { prof_bonus.get() } else { 0 };
                                    format_bonus(bonus)
                                }}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>

                // -- Skills (tracks abilities, skills, classes) --
                <h4 class="summary-subsection-title">{move_tr!("panel-skills")}</h4>
                <div class="summary-saves-grid">
                    {Skill::iter().map(|skill| {
                        let tr_key = skill.tr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        let prof_level = move || {
                            store.skills().read().get(&skill).copied().unwrap_or(ProficiencyLevel::None)
                        };
                        let proficient = move || prof_level() != ProficiencyLevel::None;
                        view! {
                            <div class="summary-save" class:proficient=proficient>
                                <span class="summary-save-label">{label}</span>
                                <span class="summary-save-value">{move || {
                                    let score = store.abilities().get().get(skill.ability()) as i32;
                                    let modifier = (score - 10).div_euclid(2);
                                    let bonus = modifier + prof_level().multiplier() * prof_bonus.get();
                                    format_bonus(bonus)
                                }}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>

                // -- Spell slots (tracks only spell_slots) --
                {move || {
                    let spell_slots = store.spell_slots().read();
                    let slots: Vec<_> = (1..=9u32)
                        .filter_map(|level| {
                            let idx = (level - 1) as usize;
                            let slot = spell_slots.get(idx).copied().unwrap_or_default();
                            if slot.total > 0 { Some((level, idx, slot)) } else { None }
                        })
                        .collect();
                    if slots.is_empty() {
                        return ().into_any();
                    }
                    view! {
                        <h4 class="summary-subsection-title">{move_tr!("spell-slots")}</h4>
                        <div class="summary-spell-slots">
                            {slots.into_iter().map(|(level, idx, slot)| {
                                let remaining = slot.total.saturating_sub(slot.used);
                                view! {
                                    <div class="summary-slot">
                                        <span class="summary-slot-level">
                                            {tr!("slot-level", {"level" => level.to_string()})}
                                        </span>
                                        <input
                                            type="number"
                                            class="short-input"
                                            min="0"
                                            prop:max=slot.total.to_string()
                                            prop:value=slot.used.to_string()
                                            on:input=move |e| {
                                                if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                    store.spell_slots().update(|slots| {
                                                        slots[idx].used = value;
                                                    });
                                                }
                                            }
                                        />
                                        <span>"/" {slot.total} " (" {remaining} ")"</span>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </div>

            // === Section: Backpack ===
            <div class="summary-section">
                <h3 class="summary-section-title">{move_tr!("summary-backpack")}</h3>

                // -- Items (tracks only equipment.items) --
                {move || {
                    let items = store.equipment().items().read();
                    let items: Vec<_> = items.iter()
                        .enumerate()
                        .filter(|(_, item)| !item.name.is_empty())
                        .map(|(idx, item)| (idx, item.name.clone(), item.quantity, item.description.clone()))
                        .collect();
                    if items.is_empty() {
                        view! {
                            <p class="summary-empty">{move_tr!("summary-no-items")}</p>
                        }.into_any()
                    } else {
                        let expanded = RwSignal::new(HashSet::<usize>::new());
                        view! {
                            <div class="summary-spells-list">
                                {items.into_iter().map(|(idx, name, qty, desc)| {
                                    let is_open = Signal::derive(move || expanded.get().contains(&idx));
                                    view! {
                                        <div class="summary-spell-entry">
                                            <div class="summary-spell-row">
                                                <ToggleButton
                                                    expanded=is_open
                                                    on_toggle=move || expanded.update(|set| { if !set.remove(&idx) { set.insert(idx); } })
                                                />
                                                <span class="summary-spell-name">{name}</span>
                                                <span class="summary-spell-level-label">
                                                    "\u{00d7}"
                                                    <input
                                                        type="number"
                                                        class="summary-qty-input"
                                                        min="0"
                                                        prop:value=qty.to_string()
                                                        on:input=move |e| {
                                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                                store.equipment().items().write()[idx].quantity = value;
                                                            }
                                                        }
                                                    />
                                                </span>
                                            </div>
                                            <Show when=move || is_open.get()>
                                                <textarea
                                                    class="summary-item-desc"
                                                    prop:value=desc.clone()
                                                    on:input=move |e| {
                                                        let value = event_target_value(&e);
                                                        store.equipment().items().write()[idx].description = value;
                                                    }
                                                />
                                            </Show>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                }}

                // -- Currency (tracks only equipment.currency) --
                <div class="summary-currency">
                    <label>{move_tr!("currency")}</label>
                    <span>{move || store.equipment().currency().get().to_string()}</span>
                </div>
            </div>
        </div>
    }
}

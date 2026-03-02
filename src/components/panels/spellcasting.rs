use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{datalist_input::DatalistInput, panel::Panel, toggle_button::ToggleButton},
    model::{
        Ability, Character, CharacterIdentity, CharacterStoreFields, Spell, Translatable,
        format_bonus,
    },
    rules::RulesRegistry,
};

#[component]
fn FeatureSpellcastingSection(
    #[prop(into)] feature_name: String,
    default_ability: Ability,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();

    // Resolve feature name → label for display title
    let panel_title = {
        let identity = store.get_untracked().identity.clone();
        registry
            .with_feature(&identity, &feature_name, |f| f.label().to_string())
            .unwrap_or_else(|| feature_name.clone())
    };
    let fname = StoredValue::new(feature_name);

    let casting_ability = Memo::new(move |_| {
        fname.with_value(|key| {
            store
                .feature_data()
                .read()
                .get(key)
                .and_then(|e| e.spells.as_ref())
                .map(|sc| sc.casting_ability)
                .unwrap_or(default_ability)
        })
    });
    let spell_save_dc = Memo::new(move |_| store.read().spell_save_dc(casting_ability.get()));
    let spell_attack = Memo::new(move |_| store.read().spell_attack_bonus(casting_ability.get()));

    let spells_expanded = RwSignal::new(HashSet::<usize>::new());

    // Reactively resolve spell list for datalist suggestions (re-runs when
    // spell_list_cache populates after async fetch)
    let spell_suggestions = Memo::new(move |_| {
        registry.track_spell_cache();
        fname.with_value(|key| resolve_feature_spell_list(&registry, &store.read().identity, key))
    });

    view! {
        <div class="spellcasting-section">
            <h4 class="skill-group-header">{panel_title}</h4>

            <div class="spell-header">
                <div class="spell-stat">
                    <label>{move_tr!("casting-ability")}</label>
                    <select
                        on:change=move |e| {
                            let value = event_target_value(&e);
                            if let Ok(ability) = serde_json::from_str::<Ability>(&value) {
                                fname.with_value(|key| {
                                    store.feature_data().update(|map| {
                                        if let Some(sc) = map.get_mut(key).and_then(|e| e.spells.as_mut()) {
                                            sc.casting_ability = ability;
                                        }
                                    });
                                });
                            }
                        }
                    >
                        {Ability::iter()
                            .map(|ability| {
                                let tr_key = ability.tr_key();
                                let option_value = (ability as u8).to_string();
                                let selected = ability == default_ability;
                                let label = Signal::derive(move || i18n.tr(tr_key));
                                view! {
                                    <option value=option_value selected=selected>
                                        {label}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                </div>
                <div class="spell-stat">
                    <label>{move_tr!("spell-save-dc")}</label>
                    <span class="computed-value">
                        {move || spell_save_dc.get().to_string()}
                    </span>
                </div>
                <div class="spell-stat">
                    <label>{move_tr!("spell-attack")}</label>
                    <span class="computed-value">
                        {move || format_bonus(spell_attack.get())}
                    </span>
                </div>
            </div>

            <div class="section-header">
                <h4>{move_tr!("spells")}</h4>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        fname.with_value(|key| {
                            store.feature_data().update(|map| {
                                if let Some(sc) = map.get_mut(key).and_then(|e| e.spells.as_mut()) {
                                    sc.spells.sort_by(|a, b| {
                                        b.sticky
                                            .cmp(&a.sticky)
                                            .then_with(|| a.level.cmp(&b.level))
                                            .then_with(|| {
                                                a.name.to_lowercase().cmp(&b.name.to_lowercase())
                                            })
                                    });
                                }
                            });
                        });
                    }
                >
                    "\u{21C5}"
                </button>
            </div>
            <div class="spells-list">
                {move || {
                    let spell_list = fname.with_value(|key| {
                        store.feature_data().read()
                            .get(key)
                            .and_then(|e| e.spells.as_ref())
                            .map(|sc| sc.spells.clone())
                            .unwrap_or_default()
                    });
                    let suggestions = spell_suggestions.get();
                    spell_list
                        .into_iter()
                        .enumerate()
                        .map(|(i, spell)| {
                            let spell_name = spell.label().to_string();
                            let spell_level = spell.level.to_string();
                            let spell_prepared = spell.prepared;
                            let spell_sticky = spell.sticky;
                            let spell_desc = spell.description.clone();
                            let is_open = Signal::derive(move || spells_expanded.get().contains(&i));
                            let options: Vec<(String, String, String)> = suggestions
                                .iter()
                                .filter(|(l, _, _, _)| *l == spell.level)
                                .map(|(_, name, label, desc)| (name.clone(), label.clone(), desc.clone()))
                                .collect();
                            view! {
                                <div class="spell-entry">
                                    <ToggleButton
                                        expanded=is_open
                                        on_toggle=move || spells_expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                    />
                                    <label class="spell-prepared">
                                        <input
                                            type="checkbox"
                                            prop:checked=spell_prepared || spell_sticky
                                            prop:disabled=spell_sticky
                                            on:change=move |_| {
                                                if !spell_sticky {
                                                    fname.with_value(|key| {
                                                        store.feature_data().update(|map| {
                                                            if let Some(sc) = map.get_mut(key).and_then(|e| e.spells.as_mut())
                                                                && let Some(spell) = sc.spells.get_mut(i)
                                                            {
                                                                spell.prepared = !spell.prepared;
                                                            }
                                                        });
                                                    });
                                                }
                                            }
                                        />
                                    </label>
                                    <DatalistInput
                                        value=spell_name
                                        placeholder=move_tr!("spell-name").get()
                                        class="spell-name"
                                        options=options
                                        on_input=move |input, resolved| {
                                            fname.with_value(|key| {
                                                store.feature_data().update(|map| {
                                                    if let Some(sc) = map.get_mut(key).and_then(|e| e.spells.as_mut())
                                                        && let Some(spell) = sc.spells.get_mut(i)
                                                    {
                                                        spell.name = resolved.unwrap_or(input);
                                                        spell.label = None;
                                                        spell.description.clear();
                                                    }
                                                });
                                            });
                                        }
                                    />
                                    <input
                                        type="number"
                                        class="short-input"
                                        min="0"
                                        max="9"
                                        placeholder="Lv"
                                        prop:value=spell_level
                                        on:input=move |e| {
                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                fname.with_value(|key| {
                                                    store.feature_data().update(|map| {
                                                        if let Some(sc) = map.get_mut(key).and_then(|e| e.spells.as_mut())
                                                            && let Some(spell) = sc.spells.get_mut(i)
                                                        {
                                                            spell.level = value;
                                                        }
                                                    });
                                                });
                                            }
                                        }
                                    />
                                    <Show when=move || !spell_sticky>
                                        <button
                                            class="btn-remove"
                                            on:click=move |_| {
                                                fname.with_value(|key| {
                                                    store.feature_data().update(|map| {
                                                        if let Some(sc) = map.get_mut(key).and_then(|e| e.spells.as_mut())
                                                            && i < sc.spells.len()
                                                        {
                                                            sc.spells.remove(i);
                                                        }
                                                    });
                                                });
                                            }
                                        >
                                            "X"
                                        </button>
                                    </Show>
                                    <Show when=move || is_open.get()>
                                        <textarea
                                            class="spell-desc"
                                            placeholder=move_tr!("description")
                                            prop:value=spell_desc.clone()
                                            on:change=move |e| {
                                                fname.with_value(|key| {
                                                    store.feature_data().update(|map| {
                                                        if let Some(sc) = map.get_mut(key).and_then(|e| e.spells.as_mut())
                                                            && let Some(spell) = sc.spells.get_mut(i)
                                                        {
                                                            spell.description = event_target_value(&e);
                                                        }
                                                    });
                                                });
                                            }
                                        />
                                    </Show>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button
                class="btn-add"
                on:click=move |_| {
                    fname.with_value(|key| {
                        store.feature_data().update(|map| {
                            if let Some(sc) = map.get_mut(key).and_then(|e| e.spells.as_mut()) {
                                sc.spells.push(Spell::default());
                            }
                        });
                    });
                }
            >
                {move_tr!("btn-add-spell")}
            </button>
        </div>
    }
}

/// Resolve the spell list for a given feature name using registry's
/// find_feature. Returns (level, name, label, description) tuples.
fn resolve_feature_spell_list(
    registry: &RulesRegistry,
    identity: &CharacterIdentity,
    feature_name: &str,
) -> Vec<(u32, String, String, String)> {
    registry
        .with_feature(identity, feature_name, |feat| {
            let spells_def = feat.spells.as_ref()?;
            Some(registry.with_spell_list(&spells_def.list, |spells| {
                spells
                    .iter()
                    .map(|spell| {
                        (
                            spell.level,
                            spell.name.clone(),
                            spell.label().to_string(),
                            spell.description.clone(),
                        )
                    })
                    .collect()
            }))
        })
        .flatten()
        .unwrap_or_default()
}

#[component]
pub fn SpellcastingPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let has_spells = Memo::new(move |_| {
        store
            .feature_data()
            .read()
            .values()
            .any(|e| e.spells.is_some())
    });
    let slots_expanded = RwSignal::new(false);

    view! {
        <Show when=move || has_spells.get()>
            <Panel title=move_tr!("panel-spellcasting") class="spellcasting-panel">
                <div class="section-header">
                    <ToggleButton
                        expanded=Signal::derive(move || slots_expanded.get())
                        on_toggle=move || slots_expanded.update(|expanded| *expanded = !*expanded)
                    />
                    <h4>{move_tr!("spell-slots")}</h4>
                </div>
                <div class="spell-slots-grid">
                    {move || {
                        let expanded = slots_expanded.get();
                        let slots: Vec<_> = store.read().all_spell_slots().collect();
                        slots
                            .into_iter()
                            .filter(|(_, slot)| expanded || slot.total > 0)
                            .map(|(level, slot)| {
                                let idx = (level - 1) as usize;
                                view! {
                                    <div class="spell-slot-entry">
                                        <span class="slot-level">"Lv " {level}</span>
                                        <input
                                            type="number"
                                            class="short-input"
                                            min="0"
                                            placeholder=move_tr!("used")
                                            prop:value=slot.used.to_string()
                                            on:input=move |e| {
                                                if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                    store.spell_slots().update(|slots| {
                                                        slots[idx].used = value;
                                                    });
                                                }
                                            }
                                        />
                                        <span>"/"</span>
                                        <input
                                            type="number"
                                            class="short-input"
                                            min="0"
                                            placeholder=move_tr!("total")
                                            prop:value=slot.total.to_string()
                                            on:input=move |e| {
                                                if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                    store.spell_slots().update(|slots| {
                                                        slots[idx].total = value;
                                                    });
                                                }
                                            }
                                        />
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>
                {move || {
                    store
                        .feature_data()
                        .read()
                        .iter()
                        .filter_map(|(name, entry)| {
                            entry
                                .spells
                                .as_ref()
                                .map(|sc| (name.clone(), sc.casting_ability))
                        })
                        .collect::<Vec<_>>()
                        .into_iter()
                        .map(|(feature_name, default_ability)| {
                            view! {
                                <FeatureSpellcastingSection feature_name=feature_name default_ability=default_ability />
                            }
                        })
                        .collect_view()
                }}
            </Panel>
        </Show>
    }
}

use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{
        datalist_input::DatalistInput, icon::Icon, panel::Panel, toggle_button::ToggleButton,
    },
    model::{
        Ability, Character, CharacterIdentity, CharacterStoreFields, Spell, SpellSlotPool,
        Translatable, format_bonus,
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

    // Per-level spell suggestions, reactively updated when spell cache loads
    let spell_suggestions: [RwSignal<Vec<(String, String, String)>>; 10] =
        std::array::from_fn(|_| RwSignal::new(Vec::new()));
    Effect::new(move || {
        registry.track_spell_cache();
        let mut data = fname
            .with_value(|key| resolve_feature_spell_list(&registry, &store.read().identity, key));
        for (level, signal) in spell_suggestions.iter().enumerate() {
            signal.set(std::mem::take(&mut data[level]));
        }
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
                    <Icon name="arrow-down-a-z" />
                </button>
            </div>
            <div class="spells-list">
                {move || {
                    let guard = store.feature_data().read();
                    fname.with_value(|key| {
                        guard
                            .get(key)
                            .and_then(|e| e.spells.as_ref())
                    }).map(|sc| sc.spells
                        .iter()
                        .enumerate()
                        .map(|(i, spell)| {
                            let spell_name = spell.label().to_string();
                            let spell_level = spell.level.to_string();
                            let spell_prepared = spell.prepared;
                            let spell_sticky = spell.sticky;
                            let is_open = Signal::derive(move || spells_expanded.get().contains(&i));
                            let options = spell_suggestions[spell.level.min(9) as usize];
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
                                        placeholder=move_tr!("spell-name")
                                        class="spell-name"
                                        options=options
                                        on_input=move |input, resolved| {
                                            fname.with_value(|key| {
                                                store.feature_data().update(|map| {
                                                    if let Some(sc) = map.get_mut(key).and_then(|e| e.spells.as_mut())
                                                        && let Some(spell) = sc.spells.get_mut(i)
                                                    {
                                                        if let Some(name) = resolved {
                                                            spell.name = name;
                                                            spell.label = Some(input);
                                                        } else {
                                                            spell.name = input;
                                                            spell.label = None;
                                                        }
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
                                            <Icon name="x" size=14 />
                                        </button>
                                    </Show>
                                    <Show when=move || is_open.get()>
                                        <textarea
                                            class="spell-desc"
                                            placeholder=move_tr!("description")
                                            prop:value=fname.with_value(|key| {
                                                store.feature_data().read()
                                                    .get(key)
                                                    .and_then(|e| e.spells.as_ref())
                                                    .and_then(|sc| sc.spells.get(i))
                                                    .map(|s| s.description.clone())
                                                    .unwrap_or_default()
                                            })
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
                        .collect_view())
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

/// Resolve the spell list for a given feature into per-level buckets.
fn resolve_feature_spell_list(
    registry: &RulesRegistry,
    identity: &CharacterIdentity,
    feature_name: &str,
) -> [Vec<(String, String, String)>; 10] {
    registry
        .with_feature(identity, feature_name, |feat| {
            let spells_def = feat.spells.as_ref()?;
            Some(registry.with_spell_list(&spells_def.list, |spells| {
                let mut by_level: [Vec<(String, String, String)>; 10] = Default::default();
                for spell in spells {
                    if let Some(bucket) = by_level.get_mut(spell.level as usize) {
                        bucket.push((
                            spell.name.clone(),
                            spell.label().to_string(),
                            spell.description.clone(),
                        ));
                    }
                }
                by_level
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
                {move || {
                    let expanded = slots_expanded.get();
                    let character = store.read();
                    let pools: Vec<SpellSlotPool> = character.active_pools().collect();
                    let multiple_pools = pools.len() > 1;
                    let i18n = expect_context::<leptos_fluent::I18n>();
                    pools
                        .into_iter()
                        .map(|pool| {
                            let slots: Vec<_> = character.all_spell_slots_for_pool(pool).collect();
                            let pool_header = if multiple_pools {
                                Some(view! {
                                    <h5 class="pool-header">{i18n.tr(pool.tr_key())}</h5>
                                })
                            } else {
                                None
                            };
                            view! {
                                {pool_header}
                                <div class="spell-slots-grid">
                                    {slots
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
                                                                store.spell_slots().update(|pools| {
                                                                    if let Some(slots) = pools.get_mut(&pool) {
                                                                        slots[idx].used = value;
                                                                    }
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
                                                                store.spell_slots().update(|pools| {
                                                                    if let Some(slots) = pools.get_mut(&pool) {
                                                                        slots[idx].total = value;
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    />
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                        })
                        .collect_view()
                }}
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

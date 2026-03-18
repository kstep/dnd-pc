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
        Ability, Character, CharacterIdentity, CharacterStoreFields, Spell, SpellData,
        SpellSlotPool, Translatable, format_bonus,
    },
    rules::RulesRegistry,
};

fn update_spells(
    fname: StoredValue<String>,
    store: Store<Character>,
    f: impl FnOnce(&mut SpellData),
) {
    fname.with_value(|key| {
        store.feature_data().update(|map| {
            if let Some(sc) = map.get_mut(key).and_then(|entry| entry.spells.as_mut()) {
                f(sc);
            }
        });
    });
}

fn update_spell(
    fname: StoredValue<String>,
    store: Store<Character>,
    index: usize,
    f: impl FnOnce(&mut Spell),
) {
    update_spells(fname, store, |sc| {
        if let Some(spell) = sc.spells.get_mut(index) {
            f(spell);
        }
    });
}

fn read_spell<T: Default>(
    fname: StoredValue<String>,
    store: Store<Character>,
    index: usize,
    f: impl FnOnce(&Spell) -> T,
) -> T {
    fname.with_value(|key| {
        store
            .feature_data()
            .read()
            .get(key)
            .and_then(|entry| entry.spells.as_ref())
            .and_then(|sc| sc.spells.get(index))
            .map(f)
            .unwrap_or_default()
    })
}

fn update_known_spell(
    fname: StoredValue<String>,
    store: Store<Character>,
    index: usize,
    f: impl FnOnce(&mut Spell),
) {
    update_spells(fname, store, |sc| {
        if let Some(known) = &mut sc.known
            && let Some(spell) = known.get_mut(index)
        {
            f(spell);
        }
    });
}

fn read_known_spell<T: Default>(
    fname: StoredValue<String>,
    store: Store<Character>,
    index: usize,
    f: impl FnOnce(&Spell) -> T,
) -> T {
    fname.with_value(|key| {
        store
            .feature_data()
            .read()
            .get(key)
            .and_then(|entry| entry.spells.as_ref())
            .and_then(|sc| sc.known.as_ref())
            .and_then(|known| known.get(index))
            .map(f)
            .unwrap_or_default()
    })
}

#[component]
fn FeatureSpellcastingSection(
    #[prop(into)] feature_name: String,
    default_ability: Ability,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();

    // Resolve feature name → label and cost suffix for display
    let identity = store.get_untracked().identity.clone();
    let panel_title = registry
        .with_feature(&identity, &feature_name, |f| f.label().to_string())
        .unwrap_or_else(|| feature_name.clone());
    let cost_short: String = registry
        .with_feature(&identity, &feature_name, |feat| {
            feat.cost_info().map(|(_, short)| short.to_string())
        })
        .flatten()
        .unwrap_or_default();
    let has_cost_field = !cost_short.is_empty();
    let cost_short = StoredValue::new(cost_short);
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
    let known_expanded = RwSignal::new(HashSet::<usize>::new());

    let is_two_tier = Memo::new(move |_| {
        fname.with_value(|key| {
            store
                .feature_data()
                .read()
                .get(key)
                .and_then(|e| e.spells.as_ref())
                .is_some_and(|sc| sc.is_two_tier())
        })
    });

    // Per-level spell suggestions from registry (for spellbook / single-tier)
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

    // Per-level suggestions from known (spellbook) entries for prepared spells
    let known_suggestions: [RwSignal<Vec<(String, String, String)>>; 10] =
        std::array::from_fn(|_| RwSignal::new(Vec::new()));
    Effect::new(move || {
        let guard = store.feature_data().read();
        let known = fname.with_value(|key| {
            guard
                .get(key)
                .and_then(|e| e.spells.as_ref())
                .and_then(|sc| sc.known.as_ref())
        });
        let mut by_level: [Vec<(String, String, String)>; 10] = Default::default();
        if let Some(known) = known {
            for spell in known.iter().filter(|s| !s.name.is_empty()) {
                if let Some(bucket) = by_level.get_mut(spell.level.min(9) as usize) {
                    bucket.push((
                        spell.name.clone(),
                        spell.label().to_string(),
                        spell.description.clone(),
                    ));
                }
            }
        }
        for (level, signal) in known_suggestions.iter().enumerate() {
            signal.set(std::mem::take(&mut by_level[level]));
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
                            if let Some(ability) = Ability::from_u8_str(&value) {
                                update_spells(fname, store, |sc| sc.casting_ability = ability);
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

            // Spellbook section (only for two-tier casters like Wizard)
            <Show when=move || is_two_tier.get()>
                <div class="section-header">
                    <h4>{move_tr!("spellbook")}</h4>
                    <button
                        class="btn-toggle-desc"
                        on:click=move |_| {
                            update_spells(fname, store, |sc| {
                                if let Some(known) = &mut sc.known {
                                    known.sort_by(|a, b| {
                                        b.sticky
                                            .cmp(&a.sticky)
                                            .then_with(|| a.level.cmp(&b.level))
                                            .then_with(|| a.name.cmp(&b.name))
                                    });
                                }
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
                                .and_then(|sc| sc.known.as_ref())
                        }).map(|known| known
                            .iter()
                            .enumerate()
                            .map(|(i, spell)| {
                                let spell_name = spell.label().to_string();
                                let spell_level = spell.level.to_string();
                                let spell_sticky = spell.sticky;
                                let is_open = Signal::derive(move || known_expanded.get().contains(&i));
                                let options = spell_suggestions[spell.level.min(9) as usize];
                                view! {
                                    <div class="spell-entry">
                                        <ToggleButton
                                            expanded=is_open
                                            on_toggle=move || known_expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                        />
                                        <DatalistInput
                                            value=spell_name
                                            placeholder=move_tr!("spell-name")
                                            class="spell-name"
                                            options=options
                                            on_input=move |input, resolved| {
                                                let desc = resolved.as_ref().and_then(|name| {
                                                    options.with(|opts| {
                                                        opts.iter()
                                                            .find(|(n, _, _)| n == name)
                                                            .map(|(_, _, d)| d.clone())
                                                    })
                                                }).unwrap_or_default();
                                                update_known_spell(fname, store, i, |spell| {
                                                    if let Some(name) = resolved {
                                                        spell.name = name;
                                                        spell.label = Some(input);
                                                    } else {
                                                        spell.set_label(input);
                                                    }
                                                    spell.description = desc;
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
                                            on:change=move |e| {
                                                if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                    update_known_spell(fname, store, i, |spell| spell.level = value);
                                                }
                                            }
                                        />
                                        <Show when=move || !spell_sticky>
                                            <button
                                                class="btn-remove"
                                                on:click=move |_| {
                                                    update_spells(fname, store, |sc| {
                                                        if let Some(known) = &mut sc.known
                                                            && i < known.len()
                                                        {
                                                            known.remove(i);
                                                        }
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
                                                prop:value=move || read_known_spell(fname, store, i, |spell| spell.description.clone())
                                                on:change=move |e| {
                                                    let value = event_target_value(&e);
                                                    update_known_spell(fname, store, i, |spell| spell.description = value);
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
                        update_spells(fname, store, |sc| {
                            if let Some(known) = &mut sc.known {
                                known.push(Spell::default());
                            }
                        });
                    }
                >
                    {move_tr!("btn-add-spell")}
                </button>
            </Show>

            // Prepared spells section (or single-tier spell list)
            <div class="section-header">
                <h4>{move || if is_two_tier.get() { move_tr!("prepared-spells") } else { move_tr!("spells") }}</h4>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        update_spells(fname, store, |sc| {
                            sc.spells.sort_by(|a, b| {
                                b.sticky
                                    .cmp(&a.sticky)
                                    .then_with(|| a.level.cmp(&b.level))
                                    .then_with(|| {
                                        a.name.cmp(&b.name)
                                    })
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
                    let two_tier = is_two_tier.get();
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
                            let spell_sticky = spell.sticky;
                            let has_free_uses = spell.free_uses.is_some();
                            let is_open = Signal::derive(move || spells_expanded.get().contains(&i));
                            // Two-tier: autocomplete from spellbook; single-tier/cantrips: from registry
                            let options = if two_tier && spell.level > 0 {
                                known_suggestions[spell.level.min(9) as usize]
                            } else {
                                spell_suggestions[spell.level.min(9) as usize]
                            };
                            view! {
                                <div class="spell-entry">
                                    <ToggleButton
                                        expanded=is_open
                                        on_toggle=move || spells_expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                    />
                                    <DatalistInput
                                        value=spell_name
                                        placeholder=move_tr!("spell-name")
                                        class="spell-name"
                                        options=options
                                        on_input=move |input, resolved| {
                                            update_spells(fname, store, |sc| {
                                                let Some(spell) = sc.spells.get_mut(i) else { return };
                                                if let Some(name) = resolved {
                                                    // Copy label/description from spellbook entry if two-tier
                                                    let known_spell = sc.known.as_ref()
                                                        .and_then(|k| k.iter().find(|s| s.name == name));
                                                    spell.description = known_spell
                                                        .map(|s| s.description.clone())
                                                        .unwrap_or_default();
                                                    spell.label = known_spell
                                                        .and_then(|s| s.label.clone())
                                                        .or(Some(input));
                                                    spell.name = name;
                                                } else {
                                                    spell.set_label(input);
                                                    spell.description.clear();
                                                }
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
                                        on:change=move |e| {
                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                update_spell(fname, store, i, |spell| spell.level = value);
                                            }
                                        }
                                    />
                                    <Show when=move || !spell_sticky>
                                        <button
                                            class="btn-remove"
                                            on:click=move |_| {
                                                update_spells(fname, store, |sc| {
                                                    if i < sc.spells.len() {
                                                        sc.spells.remove(i);
                                                    }
                                                });
                                            }
                                        >
                                            <Icon name="x" size=14 />
                                        </button>
                                    </Show>
                                    <Show when=move || is_open.get()>
                                        <Show when=move || has_free_uses || has_cost_field>
                                            <div class="spell-cost-row">
                                                <Show when=move || has_free_uses>
                                                    <span class="spell-field-group">
                                                    <span class="spell-free-uses-label">{move_tr!("free-uses")}</span>
                                                    <input
                                                        type="number"
                                                        class="short-input"
                                                        min="0"
                                                        prop:value=move || read_spell(fname, store, i, |spell| {
                                                            spell.free_uses.as_ref().map(|fu| fu.used.to_string()).unwrap_or_default()
                                                        })
                                                        on:change=move |e| {
                                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                                update_spell(fname, store, i, |spell| {
                                                                    if let Some(fu) = &mut spell.free_uses {
                                                                        fu.used = value;
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
                                                        prop:value=move || read_spell(fname, store, i, |spell| {
                                                            spell.free_uses.as_ref().map(|fu| fu.max.to_string()).unwrap_or_default()
                                                        })
                                                        on:change=move |e| {
                                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                                update_spell(fname, store, i, |spell| {
                                                                    if let Some(fu) = &mut spell.free_uses {
                                                                        fu.max = value;
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    />
                                                    </span>
                                                </Show>
                                                <span class="spell-field-group">
                                                <span class="spell-cost-label">{move_tr!("cost")}</span>
                                                <input
                                                    type="number"
                                                    class="short-input"
                                                    min="0"
                                                    prop:value=move || read_spell(fname, store, i, |spell| spell.cost.to_string())
                                                    on:change=move |e| {
                                                        if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                            update_spell(fname, store, i, |spell| spell.cost = value);
                                                        }
                                                    }
                                                />
                                                <Show when=move || has_cost_field>
                                                    <span class="spell-cost-suffix">{cost_short.get_value()}</span>
                                                </Show>
                                                </span>
                                            </div>
                                        </Show>
                                        <textarea
                                            class="spell-desc"
                                            placeholder=move_tr!("description")
                                            prop:value=move || read_spell(fname, store, i, |spell| spell.description.clone())
                                            on:change=move |e| {
                                                let value = event_target_value(&e);
                                                update_spell(fname, store, i, |spell| spell.description = value);
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
                    update_spells(fname, store, |sc| sc.spells.push(Spell::default()));
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
                for spell in spells.values() {
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
                                                        on:change=move |e| {
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
                                                        on:change=move |e| {
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

use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{panel::Panel, toggle_button::ToggleButton},
    model::{
        Ability, Character, CharacterIdentity, CharacterStoreFields, Spell, SpellData, Translatable,
    },
    rules::RulesRegistry,
};

#[component]
fn FeatureSpellcastingSection(
    #[prop(into)] feature_name: String,
    sc_data: SpellData,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();

    let fname = StoredValue::new(feature_name.clone());
    let default_ability = sc_data.casting_ability;

    let casting_ability = Memo::new(move |_| {
        store
            .feature_data()
            .read()
            .get(&fname.get_value())
            .and_then(|e| e.spells.as_ref())
            .map(|sc| sc.casting_ability)
            .unwrap_or(default_ability)
    });
    let spell_save_dc = Memo::new(move |_| store.get().spell_save_dc(casting_ability.get()));
    let spell_attack = Memo::new(move |_| store.get().spell_attack_bonus(casting_ability.get()));

    let spells_expanded = RwSignal::new(HashSet::<usize>::new());

    // Reactively resolve spell list for datalist suggestions (re-runs when
    // spell_list_cache populates after async fetch)
    let spell_suggestions = Memo::new(move |_| {
        resolve_feature_spell_list(&registry, &store.get().identity, &fname.get_value())
    });

    // Build datalist options per level (reactive)
    let datalist_prefix = feature_name.replace(' ', "-").to_lowercase();
    let datalist_prefix = StoredValue::new(datalist_prefix);

    let panel_title = feature_name;

    view! {
        <div class="spellcasting-section">
            <h4 class="skill-group-header">{panel_title}</h4>
            {move || {
                let suggestions = spell_suggestions.get();
                (0..=9u32)
                    .map(|level| {
                        let datalist_id = format!(
                            "spell-suggestions-{}-{level}",
                            datalist_prefix.get_value()
                        );
                        let options: Vec<_> = suggestions
                            .iter()
                            .filter(|(l, _, _)| *l == level)
                            .map(|(_, name, desc)| (name.clone(), desc.clone()))
                            .collect();
                        view! {
                            <datalist id=datalist_id>
                                {options
                                    .into_iter()
                                    .map(|(name, desc)| {
                                        view! { <option value=name>{desc}</option> }
                                    })
                                    .collect_view()}
                            </datalist>
                        }
                    })
                    .collect_view()
            }}

            <div class="spell-header">
                <div class="spell-stat">
                    <label>{move_tr!("casting-ability")}</label>
                    <select
                        on:change=move |e| {
                            let val = event_target_value(&e);
                            if let Ok(a) = serde_json::from_str::<Ability>(&format!("\"{val}\"")) {
                                let fname = fname.get_value();
                                store.feature_data().update(|map| {
                                    if let Some(sc) = map.get_mut(&fname).and_then(|e| e.spells.as_mut()) {
                                        sc.casting_ability = a;
                                    }
                                });
                            }
                        }
                    >
                        {Ability::iter()
                            .map(|a| {
                                let tr_key = a.tr_key();
                                let val = format!("{a:?}");
                                let selected = a == default_ability;
                                let label = Signal::derive(move || i18n.tr(tr_key));
                                view! {
                                    <option value=val selected=selected>
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
                        {move || {
                            let v = spell_attack.get();
                            if v >= 0 { format!("+{v}") } else { format!("{v}") }
                        }}
                    </span>
                </div>
            </div>

            <div class="section-header">
                <h4>{move_tr!("spells")}</h4>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        let key = fname.get_value();
                        store.feature_data().update(|map| {
                            if let Some(sc) = map.get_mut(&key).and_then(|e| e.spells.as_mut()) {
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
                    }
                >
                    "\u{21C5}"
                </button>
            </div>
            <div class="spells-list">
                {move || {
                    let key = fname.get_value();
                    let spell_list = store.feature_data().read()
                        .get(&key)
                        .and_then(|e| e.spells.as_ref())
                        .map(|sc| sc.spells.clone())
                        .unwrap_or_default();
                    spell_list
                        .into_iter()
                        .enumerate()
                        .map(|(i, spell)| {
                            let spell_name = spell.name.clone();
                            let spell_level = spell.level.to_string();
                            let spell_prepared = spell.prepared;
                            let spell_sticky = spell.sticky;
                            let spell_desc = spell.description.clone();
                            let is_open = Signal::derive(move || spells_expanded.get().contains(&i));
                            let datalist_id = format!("spell-suggestions-{}-{}", datalist_prefix.get_value(), spell.level);
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
                                                    let key = fname.get_value();
                                                    store.feature_data().update(|map| {
                                                        if let Some(sc) = map.get_mut(&key).and_then(|e| e.spells.as_mut())
                                                            && let Some(s) = sc.spells.get_mut(i)
                                                        {
                                                            s.prepared = !s.prepared;
                                                        }
                                                    });
                                                }
                                            }
                                        />
                                    </label>
                                    <input
                                        type="text"
                                        class="spell-name"
                                        list=datalist_id
                                        placeholder=move_tr!("spell-name")
                                        prop:value=spell_name
                                        on:change=move |e| {
                                            let name = event_target_value(&e);
                                            let desc = spell_suggestions.get().iter()
                                                .find(|(_, n, _)| *n == name)
                                                .map(|(_, _, d)| d.clone());
                                            let key = fname.get_value();
                                            store.feature_data().update(|map| {
                                                if let Some(sc) = map.get_mut(&key).and_then(|e| e.spells.as_mut())
                                                    && let Some(s) = sc.spells.get_mut(i)
                                                {
                                                    s.name = name;
                                                    if let Some(desc) = desc {
                                                        s.description = desc;
                                                    }
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
                                        on:input=move |e| {
                                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                let key = fname.get_value();
                                                store.feature_data().update(|map| {
                                                    if let Some(sc) = map.get_mut(&key).and_then(|e| e.spells.as_mut())
                                                        && let Some(s) = sc.spells.get_mut(i)
                                                    {
                                                        s.level = v;
                                                    }
                                                });
                                            }
                                        }
                                    />
                                    <Show when=move || !spell_sticky>
                                        <button
                                            class="btn-remove"
                                            on:click=move |_| {
                                                let key = fname.get_value();
                                                store.feature_data().update(|map| {
                                                    if let Some(sc) = map.get_mut(&key).and_then(|e| e.spells.as_mut())
                                                        && i < sc.spells.len()
                                                    {
                                                        sc.spells.remove(i);
                                                    }
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
                                                let key = fname.get_value();
                                                store.feature_data().update(|map| {
                                                    if let Some(sc) = map.get_mut(&key).and_then(|e| e.spells.as_mut())
                                                        && let Some(s) = sc.spells.get_mut(i)
                                                    {
                                                        s.description = event_target_value(&e);
                                                    }
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
                    let key = fname.get_value();
                    store.feature_data().update(|map| {
                        if let Some(sc) = map.get_mut(&key).and_then(|e| e.spells.as_mut()) {
                            sc.spells.push(Spell::default());
                        }
                    });
                }
            >
                {move_tr!("btn-add-spell")}
            </button>
        </div>
    }
}

/// Resolve the spell list for a given feature name using registry's
/// find_feature.
fn resolve_feature_spell_list(
    registry: &RulesRegistry,
    identity: &CharacterIdentity,
    feature_name: &str,
) -> Vec<(u32, String, String)> {
    registry
        .with_feature(identity, feature_name, |feat| {
            feat.spells.as_ref().map(|spells_def| {
                registry.with_spell_list(&spells_def.list, |spells| {
                    spells
                        .iter()
                        .map(|s| (s.level, s.name.clone(), s.description.clone()))
                        .collect()
                })
            })
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
                        on_toggle=move || slots_expanded.update(|v| *v = !*v)
                    />
                    <h4>{move_tr!("spell-slots")}</h4>
                </div>
                <div class="spell-slots-grid">
                    {move || {
                        let expanded = slots_expanded.get();
                        let ch = store.get();
                        let slots: Vec<_> = ch.all_spell_slots().collect();
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
                                                if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                    store.spell_slots().update(|slots| {
                                                        slots.resize_with(idx + 1, Default::default);
                                                        slots[idx].used = v;
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
                                                if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                    store.spell_slots().update(|slots| {
                                                        slots.resize_with(idx + 1, Default::default);
                                                        slots[idx].total = v;
                                                        while slots.last().is_some_and(|s| s.total == 0 && s.used == 0) {
                                                            slots.pop();
                                                        }
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
                            entry.spells.as_ref().map(|sc| (name.clone(), sc.clone()))
                        })
                        .collect::<Vec<_>>()
                        .into_iter()
                        .map(|(feature_name, sc_data)| {
                            view! {
                                <FeatureSpellcastingSection feature_name=feature_name sc_data=sc_data />
                            }
                        })
                        .collect_view()
                }}
            </Panel>
        </Show>
    }
}

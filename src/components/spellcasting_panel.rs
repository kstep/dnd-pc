use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{panel::Panel, toggle_button::ToggleButton},
    model::{
        Ability, Character, CharacterIdentityStoreFields, CharacterStoreFields, Spell,
        SpellcastingData, Translatable,
    },
    rules::RulesRegistry,
};

#[component]
pub fn SpellcastingPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let has_spellcasting = Memo::new(move |_| store.spellcasting().read().is_some());
    let casting_ability = Memo::new(move |_| {
        store
            .spellcasting()
            .read()
            .as_ref()
            .map(|sc| sc.casting_ability)
    });
    let spell_save_dc = Memo::new(move |_| store.get().spell_save_dc());
    let spell_attack = Memo::new(move |_| store.get().spell_attack_bonus());

    let registry = expect_context::<RulesRegistry>();

    let i18n = expect_context::<leptos_fluent::I18n>();
    let slots_expanded = RwSignal::new(false);
    let spells_expanded = RwSignal::new(HashSet::<usize>::new());

    view! {
        <Show when=move || has_spellcasting.get()>
        <Panel title=move_tr!("panel-spellcasting") class="spellcasting-panel">
                {move || {
                    let classes = store.identity().classes().read();
                    (0..=9u32).map(|level| {
                        let datalist_id = format!("spell-suggestions-{level}");
                        let options: Vec<(String, String)> = classes.iter().filter_map(|c| {
                            let def = registry.get_class(&c.class)?;
                            Some(def.spells(c.subclass.as_deref())
                                .filter(|s| s.level == level)
                                .map(|s| (s.name.clone(), s.description.clone()))
                                .collect::<Vec<_>>())
                        }).flatten().collect();
                        view! {
                            <datalist id=datalist_id>
                                {options.into_iter().map(|(name, desc)| {
                                    view! { <option value=name>{desc}</option> }
                                }).collect_view()}
                            </datalist>
                        }
                    }).collect_view()
                }}

                <div class="spell-header">
                    <div class="spell-stat">
                        <label>{move_tr!("casting-ability")}</label>
                        <select
                            on:change=move |e| {
                                let val = event_target_value(&e);
                                if let Ok(a) = serde_json::from_str::<Ability>(&format!("\"{val}\""))
                                    && let Some(sc) = store.spellcasting().write().as_mut()
                                {
                                    sc.casting_ability = a;
                                }
                            }
                        >
                            {Ability::iter()
                                .map(|a| {
                                    let tr_key = a.tr_key();
                                    let val = format!("{a:?}");
                                    let selected = move || casting_ability.get() == Some(a);
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
                            {move || spell_save_dc.get().map(|v| v.to_string()).unwrap_or_default()}
                        </span>
                    </div>
                    <div class="spell-stat">
                        <label>{move_tr!("spell-attack")}</label>
                        <span class="computed-value">
                            {move || {
                                spell_attack
                                    .get()
                                    .map(|v| if v >= 0 { format!("+{v}") } else { format!("{v}") })
                                    .unwrap_or_default()
                            }}
                        </span>
                    </div>
                </div>

                <div class="section-header">
                    <ToggleButton
                        expanded=Signal::derive(move || slots_expanded.get())
                        on_toggle=move || slots_expanded.update(|v| *v = !*v)
                    />
                    <h4>{move_tr!("spell-slots")}</h4>
                </div>
                <div class="spell-slots-grid">
                    {move || {
                        let sc_data = store.spellcasting().read();
                        let sc_default = SpellcastingData::default();
                        let sc = sc_data.as_ref().unwrap_or(&sc_default);
                        let expanded = slots_expanded.get();
                        sc.all_spell_slots()
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
                                                if let Ok(v) = event_target_value(&e).parse::<u32>()
                                                    && let Some(sc) = store.spellcasting().write().as_mut()
                                                    && let Some(s) = sc.spell_slots.get_mut(idx)
                                                {
                                                    s.used = v;
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
                                                if let Ok(v) = event_target_value(&e).parse::<u32>()
                                                    && let Some(sc) = store.spellcasting().write().as_mut()
                                                {
                                                    sc.spell_slots.resize_with(idx + 1, Default::default);
                                                    sc.spell_slots[idx].total = v;
                                                    while sc.spell_slots.last().is_some_and(|s| s.total == 0 && s.used == 0) {
                                                        sc.spell_slots.pop();
                                                    }
                                                }
                                            }
                                        />
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>

                <div class="section-header">
                    <h4>{move_tr!("spells")}</h4>
                    <button
                        class="btn-toggle-desc"
                        on:click=move |_| {
                            if let Some(sc) = store.spellcasting().write().as_mut() {
                                sc.spells.sort_by(|a, b| {
                                    a.level.cmp(&b.level).then_with(|| {
                                        a.name.to_lowercase().cmp(&b.name.to_lowercase())
                                    })
                                });
                            }
                        }
                    >
                        "\u{21C5}"
                    </button>
                </div>
                <div class="spells-list">
                    {move || {
                        let spell_list = store.spellcasting().read()
                            .as_ref()
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
                                                    if !spell_sticky
                                                        && let Some(sc) = store.spellcasting().write().as_mut()
                                                        && let Some(s) = sc.spells.get_mut(i)
                                                    {
                                                        s.prepared = !s.prepared;
                                                    }
                                                }
                                            />
                                        </label>
                                        <input
                                            type="text"
                                            class="spell-name"
                                            list=format!("spell-suggestions-{}", spell.level)
                                            placeholder=move_tr!("spell-name")
                                            prop:value=spell_name
                                            on:input=move |e| {
                                                let name = event_target_value(&e);
                                                let classes = store.identity().classes().read();
                                                let desc = classes.iter().find_map(|c| {
                                                    registry.get_class(&c.class).and_then(|def| {
                                                        def.spells(c.subclass.as_deref()).find(|sp| sp.name == name).map(|sp| sp.description.clone())
                                                    })
                                                });
                                                drop(classes);
                                                if let Some(sc) = store.spellcasting().write().as_mut()
                                                    && let Some(s) = sc.spells.get_mut(i)
                                                {
                                                    s.name = name;
                                                    if let Some(desc) = desc {
                                                        s.description = desc;
                                                    }
                                                }
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
                                                if let Ok(v) = event_target_value(&e).parse::<u32>()
                                                    && let Some(sc) = store.spellcasting().write().as_mut()
                                                    && let Some(s) = sc.spells.get_mut(i)
                                                {
                                                    s.level = v;
                                                }
                                            }
                                        />
                                        <Show when=move || !spell_sticky>
                                            <button
                                                class="btn-remove"
                                                on:click=move |_| {
                                                    if let Some(sc) = store.spellcasting().write().as_mut()
                                                        && i < sc.spells.len()
                                                    {
                                                        sc.spells.remove(i);
                                                    }
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
                                                    if let Some(sc) = store.spellcasting().write().as_mut()
                                                        && let Some(s) = sc.spells.get_mut(i)
                                                    {
                                                        s.description = event_target_value(&e);
                                                    }
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
                        if let Some(sc) = store.spellcasting().write().as_mut() {
                            sc.spells.push(Spell::default());
                        }
                    }
                >
                    {move_tr!("btn-add-spell")}
                </button>
        </Panel>
        </Show>
    }
}

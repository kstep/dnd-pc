use leptos::prelude::*;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::panel::Panel,
    model::{
        Ability, Character, CharacterStoreFields, MetamagicData, MetamagicOption, Spell,
        SpellcastingData,
    },
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

    let toggle_spellcasting = move |_| {
        if store.spellcasting().read().is_some() {
            store.spellcasting().set(None);
        } else {
            store.spellcasting().set(Some(SpellcastingData::default()));
        }
    };

    let has_metamagic = Memo::new(move |_| {
        store
            .spellcasting()
            .read()
            .as_ref()
            .is_some_and(|sc| sc.metamagic.is_some())
    });

    let toggle_metamagic = move |_| {
        if let Some(sc) = store.spellcasting().write().as_mut() {
            if sc.metamagic.is_some() {
                sc.metamagic = None;
            } else {
                sc.metamagic = Some(MetamagicData::default());
            }
        }
    };

    view! {
        <Panel title="Spellcasting" class="spellcasting-panel">
            <label class="toggle-row">
                <input
                    type="checkbox"
                    prop:checked=move || has_spellcasting.get()
                    on:change=toggle_spellcasting
                />
                " Enable Spellcasting"
            </label>

            <Show when=move || has_spellcasting.get()>
                <div class="spell-header">
                    <div class="spell-stat">
                        <label>"Casting Ability"</label>
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
                                    let label = a.to_string();
                                    let val = format!("{a:?}");
                                    let selected = move || casting_ability.get() == Some(a);
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
                        <label>"Spell Save DC"</label>
                        <span class="computed-value">
                            {move || spell_save_dc.get().map(|v| v.to_string()).unwrap_or_default()}
                        </span>
                    </div>
                    <div class="spell-stat">
                        <label>"Spell Attack"</label>
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

                <h4>"Spell Slots"</h4>
                <div class="spell-slots-grid">
                    {move || {
                        let slots = store.spellcasting().read()
                            .as_ref()
                            .map(|sc| sc.spell_slots.clone())
                            .unwrap_or_default();
                        slots
                            .into_iter()
                            .enumerate()
                            .map(|(i, slot)| {
                                view! {
                                    <div class="spell-slot-entry">
                                        <span class="slot-level">"Lv " {slot.level}</span>
                                        <input
                                            type="number"
                                            class="short-input"
                                            min="0"
                                            placeholder="Used"
                                            prop:value=slot.used.to_string()
                                            on:input=move |e| {
                                                if let Ok(v) = event_target_value(&e).parse::<u32>()
                                                    && let Some(sc) = store.spellcasting().write().as_mut()
                                                    && let Some(s) = sc.spell_slots.get_mut(i)
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
                                            placeholder="Total"
                                            prop:value=slot.total.to_string()
                                            on:input=move |e| {
                                                if let Ok(v) = event_target_value(&e).parse::<u32>()
                                                    && let Some(sc) = store.spellcasting().write().as_mut()
                                                    && let Some(s) = sc.spell_slots.get_mut(i)
                                                {
                                                    s.total = v;
                                                }
                                            }
                                        />
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>

                <h4>"Spells"</h4>
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
                                let spell_desc = spell.description.clone();
                                let show_desc = RwSignal::new(false);
                                view! {
                                    <div class="spell-entry">
                                        <label class="spell-prepared">
                                            <input
                                                type="checkbox"
                                                prop:checked=spell_prepared
                                                on:change=move |_| {
                                                    if let Some(sc) = store.spellcasting().write().as_mut()
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
                                            placeholder="Spell name"
                                            prop:value=spell_name
                                            on:input=move |e| {
                                                if let Some(sc) = store.spellcasting().write().as_mut()
                                                    && let Some(s) = sc.spells.get_mut(i)
                                                {
                                                    s.name = event_target_value(&e);
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
                                        <button
                                            class="btn-toggle-desc"
                                            on:click=move |_| show_desc.update(|v| *v = !*v)
                                        >
                                            {move || if show_desc.get() { "\u{2212}" } else { "+" }}
                                        </button>
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
                                        <Show when=move || show_desc.get()>
                                            <textarea
                                                class="spell-desc"
                                                placeholder="Description"
                                                prop:value=spell_desc.clone()
                                                on:input=move |e| {
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
                    "+ Add Spell"
                </button>

                <hr class="section-divider" />

                <label class="toggle-row">
                    <input
                        type="checkbox"
                        prop:checked=move || has_metamagic.get()
                        on:change=toggle_metamagic
                    />
                    " Enable Metamagic"
                </label>

                <Show when=move || has_metamagic.get()>
                    <h4>"Sorcery Points"</h4>
                    <div class="sorcery-points">
                        <span class="slot-level">"Used"</span>
                        <input
                            type="number"
                            class="short-input"
                            min="0"
                            placeholder="Used"
                            prop:value=move || {
                                store.spellcasting().read()
                                    .as_ref()
                                    .and_then(|sc| sc.metamagic.as_ref())
                                    .map(|mm| mm.sorcery_points_used.to_string())
                                    .unwrap_or_default()
                            }
                            on:input=move |e| {
                                if let Ok(v) = event_target_value(&e).parse::<u32>()
                                    && let Some(sc) = store.spellcasting().write().as_mut()
                                    && let Some(mm) = sc.metamagic.as_mut()
                                {
                                    mm.sorcery_points_used = v;
                                }
                            }
                        />
                        <span>"/"</span>
                        <input
                            type="number"
                            class="short-input"
                            min="0"
                            placeholder="Max"
                            prop:value=move || {
                                store.spellcasting().read()
                                    .as_ref()
                                    .and_then(|sc| sc.metamagic.as_ref())
                                    .map(|mm| mm.sorcery_points_max.to_string())
                                    .unwrap_or_default()
                            }
                            on:input=move |e| {
                                if let Ok(v) = event_target_value(&e).parse::<u32>()
                                    && let Some(sc) = store.spellcasting().write().as_mut()
                                    && let Some(mm) = sc.metamagic.as_mut()
                                {
                                    mm.sorcery_points_max = v;
                                }
                            }
                        />
                    </div>

                    <h4>"Metamagic"</h4>
                    <div class="metamagic-list">
                        {move || {
                            let options = store.spellcasting().read()
                                .as_ref()
                                .and_then(|sc| sc.metamagic.as_ref())
                                .map(|mm| mm.options.clone())
                                .unwrap_or_default();
                            options
                                .into_iter()
                                .enumerate()
                                .map(|(i, opt)| {
                                    let opt_name = opt.name.clone();
                                    let opt_cost = opt.cost.clone();
                                    let opt_desc = opt.description.clone();
                                    let show_desc = RwSignal::new(false);
                                    view! {
                                        <div class="metamagic-entry">
                                            <input
                                                type="text"
                                                class="metamagic-name"
                                                placeholder="Name"
                                                prop:value=opt_name
                                                on:input=move |e| {
                                                    if let Some(sc) = store.spellcasting().write().as_mut()
                                                        && let Some(mm) = sc.metamagic.as_mut()
                                                        && let Some(o) = mm.options.get_mut(i)
                                                    {
                                                        o.name = event_target_value(&e);
                                                    }
                                                }
                                            />
                                            <input
                                                type="text"
                                                class="metamagic-cost"
                                                placeholder="Cost"
                                                prop:value=opt_cost
                                                on:input=move |e| {
                                                    if let Some(sc) = store.spellcasting().write().as_mut()
                                                        && let Some(mm) = sc.metamagic.as_mut()
                                                        && let Some(o) = mm.options.get_mut(i)
                                                    {
                                                        o.cost = event_target_value(&e);
                                                    }
                                                }
                                            />
                                            <button
                                                class="btn-toggle-desc"
                                                on:click=move |_| show_desc.update(|v| *v = !*v)
                                            >
                                                {move || if show_desc.get() { "\u{2212}" } else { "+" }}
                                            </button>
                                            <button
                                                class="btn-remove"
                                                on:click=move |_| {
                                                    if let Some(sc) = store.spellcasting().write().as_mut()
                                                        && let Some(mm) = sc.metamagic.as_mut()
                                                        && i < mm.options.len()
                                                    {
                                                        mm.options.remove(i);
                                                    }
                                                }
                                            >
                                                "X"
                                            </button>
                                            <Show when=move || show_desc.get()>
                                                <textarea
                                                    class="metamagic-desc"
                                                    placeholder="Description"
                                                    prop:value=opt_desc.clone()
                                                    on:input=move |e| {
                                                        if let Some(sc) = store.spellcasting().write().as_mut()
                                                            && let Some(mm) = sc.metamagic.as_mut()
                                                            && let Some(o) = mm.options.get_mut(i)
                                                        {
                                                            o.description = event_target_value(&e);
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
                            if let Some(sc) = store.spellcasting().write().as_mut()
                                && let Some(mm) = sc.metamagic.as_mut()
                            {
                                mm.options.push(MetamagicOption::default());
                            }
                        }
                    >
                        "+ Add Metamagic"
                    </button>
                </Show>
            </Show>
        </Panel>
    }
}

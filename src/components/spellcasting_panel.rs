use leptos::prelude::*;
use strum::IntoEnumIterator;

use crate::model::{Ability, Character, Spell, SpellcastingData};

#[component]
pub fn SpellcastingPanel() -> impl IntoView {
    let char_signal = use_context::<RwSignal<Character>>().expect("Character context");

    let has_spellcasting = Memo::new(move |_| char_signal.get().spellcasting.is_some());
    let casting_ability = Memo::new(move |_| {
        char_signal
            .get()
            .spellcasting
            .as_ref()
            .map(|sc| sc.casting_ability)
    });
    let spell_save_dc = Memo::new(move |_| char_signal.get().spell_save_dc());
    let spell_attack = Memo::new(move |_| char_signal.get().spell_attack_bonus());
    let spell_slots = Memo::new(move |_| {
        char_signal
            .get()
            .spellcasting
            .as_ref()
            .map(|sc| sc.spell_slots.clone())
            .unwrap_or_default()
    });
    let spells = Memo::new(move |_| {
        char_signal
            .get()
            .spellcasting
            .as_ref()
            .map(|sc| sc.spells.clone())
            .unwrap_or_default()
    });

    let toggle_spellcasting = move |_| {
        char_signal.update(|c| {
            if c.spellcasting.is_some() {
                c.spellcasting = None;
            } else {
                c.spellcasting = Some(SpellcastingData::default());
            }
        });
    };

    view! {
        <div class="panel spellcasting-panel">
            <h3>"Spellcasting"</h3>
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
                                if let Ok(a) = serde_json::from_str::<Ability>(&format!("\"{val}\"")) {
                                    char_signal.update(|c| {
                                        if let Some(sc) = c.spellcasting.as_mut() {
                                            sc.casting_ability = a;
                                        }
                                    });
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
                        spell_slots
                            .get()
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
                                                if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                    char_signal.update(|c| {
                                                        if let Some(sc) = c.spellcasting.as_mut()
                                                            && let Some(s) = sc.spell_slots.get_mut(i)
                                                        {
                                                            s.used = v;
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
                                            placeholder="Total"
                                            prop:value=slot.total.to_string()
                                            on:input=move |e| {
                                                if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                    char_signal.update(|c| {
                                                        if let Some(sc) = c.spellcasting.as_mut()
                                                            && let Some(s) = sc.spell_slots.get_mut(i)
                                                        {
                                                            s.total = v;
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

                <h4>"Spells"</h4>
                <div class="spells-list">
                    {move || {
                        spells
                            .get()
                            .into_iter()
                            .enumerate()
                            .map(|(i, spell)| {
                                view! {
                                    <div class="spell-entry">
                                        <label class="spell-prepared">
                                            <input
                                                type="checkbox"
                                                prop:checked=spell.prepared
                                                on:change=move |_| {
                                                    char_signal.update(|c| {
                                                        if let Some(sc) = c.spellcasting.as_mut()
                                                            && let Some(s) = sc.spells.get_mut(i)
                                                        {
                                                            s.prepared = !s.prepared;
                                                        }
                                                    });
                                                }
                                            />
                                        </label>
                                        <input
                                            type="text"
                                            placeholder="Spell name"
                                            prop:value=spell.name.clone()
                                            on:input=move |e| {
                                                char_signal.update(|c| {
                                                    if let Some(sc) = c.spellcasting.as_mut()
                                                        && let Some(s) = sc.spells.get_mut(i)
                                                    {
                                                        s.name = event_target_value(&e);
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
                                            prop:value=spell.level.to_string()
                                            on:input=move |e| {
                                                if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                    char_signal.update(|c| {
                                                        if let Some(sc) = c.spellcasting.as_mut()
                                                            && let Some(s) = sc.spells.get_mut(i)
                                                        {
                                                            s.level = v;
                                                        }
                                                    });
                                                }
                                            }
                                        />
                                        <button
                                            class="btn-remove"
                                            on:click=move |_| {
                                                char_signal.update(|c| {
                                                    if let Some(sc) = c.spellcasting.as_mut()
                                                        && i < sc.spells.len()
                                                    {
                                                        sc.spells.remove(i);
                                                    }
                                                });
                                            }
                                        >
                                            "X"
                                        </button>
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>
                <button
                    class="btn-add"
                    on:click=move |_| {
                        char_signal.update(|c| {
                            if let Some(sc) = c.spellcasting.as_mut() {
                                sc.spells.push(Spell::default());
                            }
                        });
                    }
                >
                    "+ Add Spell"
                </button>
            </Show>
        </div>
    }
}

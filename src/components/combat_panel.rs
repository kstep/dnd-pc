use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::panel::Panel,
    model::{
        Ability, Character, CharacterIdentityStoreFields, CharacterStoreFields,
        CombatStatsStoreFields, FeatureValue,
    },
};

#[component]
pub fn CombatPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let combat = store.combat();
    let classes = store.identity().classes();
    let initiative = Memo::new(move |_| store.get().initiative());

    let init_display = move || {
        let i = initiative.get();
        if i >= 0 {
            format!("+{i}")
        } else {
            format!("{i}")
        }
    };

    view! {
        <Panel title=move_tr!("panel-combat") class="combat-panel">
            <div class="combat-top-row">
                <div class="combat-stat">
                    <label>{move_tr!("armor-class")}</label>
                    <input
                        type="number"
                        prop:value=move || combat.armor_class().get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<i32>() {
                                combat.armor_class().set(v);
                            }
                        }
                    />
                </div>
                <div class="combat-stat">
                    <label>{move_tr!("initiative")}</label>
                    <span class="computed-value">{init_display}</span>
                </div>
                <div class="combat-stat">
                    <label>{move_tr!("speed")}</label>
                    <input
                        type="number"
                        prop:value=move || combat.speed().get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                combat.speed().set(v);
                            }
                        }
                    />
                </div>
            </div>

            <div class="hp-section">
                <div class="hp-row">
                    <div class="combat-stat">
                        <label>{move_tr!("current-hp")}</label>
                        <input
                            type="number"
                            prop:value=move || combat.hp_current().get().to_string()
                            on:input=move |e| {
                                if let Ok(v) = event_target_value(&e).parse::<i32>() {
                                    combat.hp_current().set(v);
                                }
                            }
                        />
                    </div>
                    <div class="combat-stat">
                        <label>{move_tr!("hp-max")}</label>
                        <input
                            type="number"
                            prop:value=move || combat.hp_max().get().to_string()
                            on:input=move |e| {
                                if let Ok(v) = event_target_value(&e).parse::<i32>() {
                                    combat.hp_max().set(v);
                                }
                            }
                        />
                    </div>
                    <div class="combat-stat">
                        <label>{move_tr!("temp-hp")}</label>
                        <input
                            type="number"
                            prop:value=move || combat.hp_temp().get().to_string()
                            on:input=move |e| {
                                if let Ok(v) = event_target_value(&e).parse::<i32>() {
                                    combat.hp_temp().set(v);
                                }
                            }
                        />
                    </div>
                </div>
            </div>

            <div class="hit-dice-section">
                <h4>{move_tr!("hit-dice")}</h4>
                {move || {
                    classes
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, cl)| {
                            let class_label = if cl.class.is_empty() {
                                format!("Class {}", i + 1)
                            } else {
                                cl.class.clone()
                            };
                            let die_label = format!("d{}", cl.hit_die_sides);
                            let used_val = cl.hit_dice_used.to_string();
                            let total = cl.level.to_string();
                            let sides = cl.hit_die_sides;
                            let level = cl.level;
                            let all_used = cl.hit_dice_used >= level;
                            view! {
                                <div class="hit-dice-entry">
                                    <span class="hit-dice-class">{class_label}</span>
                                    <span class="hit-dice-die">{die_label}</span>
                                    <input
                                        type="number"
                                        class="hit-dice-used"
                                        min="0"
                                        prop:max=total.clone()
                                        prop:value=used_val
                                        on:input=move |e| {
                                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                let max = classes.read()[i].level;
                                                classes.write()[i].hit_dice_used = v.min(max);
                                            }
                                        }
                                    />
                                    <span class="hit-dice-sep">"/"</span>
                                    <span class="hit-dice-total">{total}</span>
                                    <button
                                        class="btn-spend-hd"
                                        disabled=all_used
                                        on:click=move |_| {
                                            if classes.read()[i].hit_dice_used >= level {
                                                return;
                                            }
                                            let con_mod = store.get().ability_modifier(Ability::Constitution);
                                            let roll = (js_sys::Math::random() * sides as f64).floor() as i32 + 1;
                                            let heal = (roll + con_mod).max(1);
                                            let hp = combat.hp_current().get();
                                            let max_hp = combat.hp_max().get();
                                            combat.hp_current().set((hp + heal).min(max_hp));
                                            classes.write()[i].hit_dice_used += 1;
                                        }
                                    >
                                        "\u{1F3B2}"
                                    </button>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>

            <div class="death-saves-row">
            <div class="death-saves">
                <h4>{move_tr!("death-saves")}</h4>
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

            <div class="rest-actions">
                <button
                    class="btn-rest"
                    on:click=move |_| {
                        combat.death_save_successes().set(0);
                        combat.death_save_failures().set(0);
                    }
                >
                    {move_tr!("short-rest")}
                </button>
                <button
                    class="btn-rest"
                    on:click=move |_| {
                        // Restore HP
                        combat.hp_current().set(combat.hp_max().get());
                        // Reset death saves
                        combat.death_save_successes().set(0);
                        combat.death_save_failures().set(0);
                        // Regain half spent hit dice per class
                        {
                            let mut w = classes.write();
                            for cl in w.iter_mut() {
                                let regain = (cl.level / 2).max(1).min(cl.hit_dice_used);
                                cl.hit_dice_used -= regain;
                            }
                        }
                        // Reset spell slots
                        store.spell_slots().update(|slots| {
                            for slot in slots.iter_mut() {
                                slot.used = 0;
                            }
                        });
                        // Reset feature points (sorcery points, etc.)
                        store.fields().update(|fields| {
                            for field_list in fields.values_mut() {
                                for field in field_list.iter_mut() {
                                    if let FeatureValue::Points { used, .. } = &mut field.value {
                                        *used = 0;
                                    }
                                }
                            }
                        });
                    }
                >
                    {move_tr!("long-rest")}
                </button>
            </div>
            </div>
        </Panel>
    }
}

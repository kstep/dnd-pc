use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{icon::Icon, panel::Panel},
    model::{
        Character, CharacterIdentityStoreFields, CharacterStoreFields, CombatStatsStoreFields,
        format_bonus,
    },
    rules::RulesRegistry,
};

#[component]
pub fn CombatPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let combat = store.combat();
    let classes = store.identity().classes();
    let initiative = Memo::new(move |_| store.read().initiative());

    let init_display = move || format_bonus(initiative.get());

    view! {
        <Panel title=move_tr!("panel-combat") class="combat-panel">
            <div class="combat-top-row">
                <div class="combat-stat">
                    <label>{move_tr!("armor-class")}</label>
                    <div class="combat-stat-row">
                        <input
                            type="number"
                            prop:value=move || store.read().armor_class()
                            on:input=move |e| {
                                if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                    combat.armor_class().set(value);
                                }
                            }
                        />
                    </div>
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
                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                combat.speed().set(value);
                            }
                        }
                    />
                </div>
                <div class="combat-stat">
                    <label>{move_tr!("attack-count")}</label>
                    <input
                        type="number"
                        min="1"
                        prop:value=move || combat.attack_count().get().to_string()
                        on:input=move |event| {
                            if let Ok(value) = event_target_value(&event).parse::<u32>() {
                                combat.attack_count().set(value.max(1));
                            }
                        }
                    />
                </div>
                <div class="combat-stat">
                    <label>{move_tr!("inspiration")}</label>
                    <button
                        class="inspiration-toggle"
                        class:active=move || combat.inspiration().get()
                        on:click=move |_| {
                            combat.inspiration().update(|v| *v = !*v);
                        }
                    >
                        {move || if combat.inspiration().get() { "\u{2605}" } else { "\u{2606}" }}
                    </button>
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
                                if let Ok(value) = event_target_value(&e).parse() {
                                    combat.hp_current().set(value);
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
                                if let Ok(value) = event_target_value(&e).parse() {
                                    combat.hp_max().set(value);
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
                                if let Ok(value) = event_target_value(&e).parse() {
                                    combat.hp_temp().set(value);
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
                        .map(|(i, class)| {
                            let class_label = if class.class.is_empty() {
                                format!("Class {}", i + 1)
                            } else {
                                class.class_label().to_string()
                            };
                            let die_label = format!("d{}", class.hit_die_sides);
                            let used_val = class.hit_dice_used;
                            let total = class.level;
                            view! {
                                <div class="hit-dice-entry">
                                    <span class="hit-dice-class">{class_label}</span>
                                    <span class="hit-dice-die">{die_label}</span>
                                    <input
                                        type="number"
                                        class="hit-dice-used"
                                        min="0"
                                        prop:max=total
                                        prop:value=used_val
                                        on:input=move |e| {
                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                let max = classes.read()[i].level;
                                                classes.write()[i].hit_dice_used = value.min(max);
                                            }
                                        }
                                    />
                                    <span class="hit-dice-sep">"/"</span>
                                    <span class="hit-dice-total">{total}</span>
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
                        store.update(|ch| ch.long_rest());
                    }
                >
                    <Icon name="list-restart" size=14 />
                    " "
                    {move_tr!("reset-stats")}
                </button>
                <button
                    class="btn-rest"
                    title=move_tr!("recalculate")
                    on:click=move |_| {
                        store.update(|ch| registry.compute(ch));
                    }
                >
                    <Icon name="refresh-cw" size=14 />
                    " "
                    {move_tr!("recalculate")}
                </button>
                <button
                    class="btn-rest"
                    style="display:none"
                    title=move_tr!("replay")
                    on:click=move |_| {
                        let window = web_sys::window().unwrap();
                        if window.confirm_with_message("Replay will reset and re-apply all features. Continue?").unwrap_or(false) {
                            store.update(|ch| registry.replay(ch));
                        }
                    }
                >
                    <Icon name="rotate-ccw" size=14 />
                    " "
                    {move_tr!("replay")}
                </button>
            </div>
            </div>
        </Panel>
    }
}

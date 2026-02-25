use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::{dice_input::DiceInput, panel::Panel},
    model::{Character, CharacterStoreFields, CombatStatsStoreFields},
};

#[component]
pub fn CombatPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let combat = store.combat();
    let initiative = Memo::new(move |_| store.get().initiative());
    let hit_dice_total = Memo::new(move |_| combat.hit_dice_total().get());
    let hit_dice_remaining = Memo::new(move |_| combat.hit_dice_remaining().get());

    let init_display = move || {
        let i = initiative.get();
        if i >= 0 {
            format!("+{i}")
        } else {
            format!("{i}")
        }
    };

    view! {
        <Panel title="Combat" class="combat-panel">
            <div class="combat-top-row">
                <div class="combat-stat">
                    <label>"Armor Class"</label>
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
                    <label>"Initiative"</label>
                    <span class="computed-value">{init_display}</span>
                </div>
                <div class="combat-stat">
                    <label>"Speed"</label>
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
                        <label>"HP Max"</label>
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
                        <label>"Current HP"</label>
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
                        <label>"Temp HP"</label>
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
                <div class="dice-field">
                    <label>"Hit Dice Total"</label>
                    <DiceInput
                        value=hit_dice_total
                        on_change=move |d| combat.hit_dice_total().set(d)
                    />
                </div>
                <div class="dice-field">
                    <label>"Hit Dice Remaining"</label>
                    <DiceInput
                        value=hit_dice_remaining
                        on_change=move |d| combat.hit_dice_remaining().set(d)
                    />
                </div>
            </div>

            <div class="death-saves">
                <h4>"Death Saves"</h4>
                <div class="death-save-row">
                    <span>"Successes"</span>
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
                    <span>"Failures"</span>
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
        </Panel>
    }
}

use leptos::prelude::*;

use crate::model::Character;

#[component]
pub fn CombatPanel() -> impl IntoView {
    let char_signal = use_context::<RwSignal<Character>>().expect("Character context");

    let ac = Memo::new(move |_| char_signal.get().combat.armor_class);
    let initiative = Memo::new(move |_| char_signal.get().initiative());
    let speed = Memo::new(move |_| char_signal.get().combat.speed);
    let hp_max = Memo::new(move |_| char_signal.get().combat.hp_max);
    let hp_current = Memo::new(move |_| char_signal.get().combat.hp_current);
    let hp_temp = Memo::new(move |_| char_signal.get().combat.hp_temp);
    let hit_dice_total = Memo::new(move |_| char_signal.get().combat.hit_dice_total.clone());
    let hit_dice_remaining =
        Memo::new(move |_| char_signal.get().combat.hit_dice_remaining.clone());
    let death_successes = Memo::new(move |_| char_signal.get().combat.death_save_successes);
    let death_failures = Memo::new(move |_| char_signal.get().combat.death_save_failures);

    let init_display = move || {
        let i = initiative.get();
        if i >= 0 {
            format!("+{i}")
        } else {
            format!("{i}")
        }
    };

    view! {
        <div class="panel combat-panel">
            <h3>"Combat"</h3>
            <div class="combat-top-row">
                <div class="combat-stat">
                    <label>"Armor Class"</label>
                    <input
                        type="number"
                        prop:value=move || ac.get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<i32>() {
                                char_signal.update(|c| c.combat.armor_class = v);
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
                        prop:value=move || speed.get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                char_signal.update(|c| c.combat.speed = v);
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
                            prop:value=move || hp_max.get().to_string()
                            on:input=move |e| {
                                if let Ok(v) = event_target_value(&e).parse::<i32>() {
                                    char_signal.update(|c| c.combat.hp_max = v);
                                }
                            }
                        />
                    </div>
                    <div class="combat-stat">
                        <label>"Current HP"</label>
                        <input
                            type="number"
                            prop:value=move || hp_current.get().to_string()
                            on:input=move |e| {
                                if let Ok(v) = event_target_value(&e).parse::<i32>() {
                                    char_signal.update(|c| c.combat.hp_current = v);
                                }
                            }
                        />
                    </div>
                    <div class="combat-stat">
                        <label>"Temp HP"</label>
                        <input
                            type="number"
                            prop:value=move || hp_temp.get().to_string()
                            on:input=move |e| {
                                if let Ok(v) = event_target_value(&e).parse::<i32>() {
                                    char_signal.update(|c| c.combat.hp_temp = v);
                                }
                            }
                        />
                    </div>
                </div>
            </div>

            <div class="hit-dice-section">
                <div class="combat-stat">
                    <label>"Hit Dice Total"</label>
                    <input
                        type="text"
                        prop:value=hit_dice_total
                        on:input=move |e| {
                            char_signal.update(|c| c.combat.hit_dice_total = event_target_value(&e));
                        }
                    />
                </div>
                <div class="combat-stat">
                    <label>"Hit Dice Remaining"</label>
                    <input
                        type="text"
                        prop:value=hit_dice_remaining
                        on:input=move |e| {
                            char_signal.update(|c| c.combat.hit_dice_remaining = event_target_value(&e));
                        }
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
                                let checked = move || death_successes.get() > i;
                                view! {
                                    <button
                                        class="death-save-box"
                                        class:filled=checked
                                        on:click=move |_| {
                                            char_signal.update(|c| {
                                                if c.combat.death_save_successes > i {
                                                    c.combat.death_save_successes = i;
                                                } else {
                                                    c.combat.death_save_successes = i + 1;
                                                }
                                            });
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
                                let checked = move || death_failures.get() > i;
                                view! {
                                    <button
                                        class="death-save-box"
                                        class:filled=checked
                                        on:click=move |_| {
                                            char_signal.update(|c| {
                                                if c.combat.death_save_failures > i {
                                                    c.combat.death_save_failures = i;
                                                } else {
                                                    c.combat.death_save_failures = i + 1;
                                                }
                                            });
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
        </div>
    }
}

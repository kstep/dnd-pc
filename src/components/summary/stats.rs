use leptos::{html::Input, prelude::*};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::icon::Icon,
    model::{
        Ability, Character, CharacterStoreFields, CombatStatsStoreFields, Skill, Translatable,
        format_bonus,
    },
};

#[component]
pub fn StatsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<I18n>();

    let combat = store.combat();
    let initiative = Memo::new(move |_| store.read().initiative());
    let damage_input = NodeRef::<Input>::new();
    let damage_value = move || {
        damage_input
            .read()
            .as_ref()
            .and_then(|input| {
                let value = input.value().parse::<u32>().ok()?;
                input.set_value("");
                Some(value)
            })
            .unwrap_or_default()
    };

    move || {
        view! {
            <div class="summary-section summary-section-stats">
                <h3 class="summary-section-title">{move_tr!("summary-stats")}</h3>

                // -- HP --
                <div class="summary-core-stats">
                    <div class="summary-stat-box summary-hp-box">
                        <label>{move_tr!("hp")}</label>
                        <div class="summary-hp-controls">
                            <div class="summary-hp-damage">
                                <input type="number" min="1" required class="summary-damage-input" node_ref=damage_input />
                                <div class="btn-container">
                                    <button class="btn-icon btn-icon--danger" title=move_tr!("damage")
                                        on:click=move |_| {
                                            let damage = damage_value();
                                            if damage > 0 {
                                                combat.update(|c| c.damage(damage));
                                            }
                                        }
                                    ><Icon name="swords" size=14 /></button>
                                    <button class="btn-icon btn-icon--success" title=move_tr!("heal")
                                        on:click=move |_| {
                                            let heal = damage_value();
                                            if heal > 0 {
                                                combat.update(|c| c.heal(heal));
                                            }
                                        }
                                    ><Icon name="heart-plus" size=14 /></button>
                                </div>
                            </div>
                            <div class="summary-hp-value">
                                {move || combat.hp_current().get()}
                                " ("
                                <input type="number" min="0" class="summary-hp-temp-input" prop:value=move || combat.hp_temp().get()
                                    on:change=move |event| {
                                        let value = event_target_value(&event).parse().unwrap_or_default();
                                        combat.hp_temp().set(value);
                                    }
                                />
                                ")"
                                <span class="summary-hp-max">
                                    "/ " {move || combat.hp_max().get()}
                                </span>
                            </div>
                        </div>
                    </div>
                    // -- Inspiration toggle --
                    <div class="summary-stat-box summary-inspiration-box">
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

                // -- Death saves (shown when HP == 0) --
                <Show when=move || combat.hp_current().get() == 0>
                    <div class="summary-death-saves">
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
                </Show>

                // -- Core stats: AC, Initiative, Speed --
                <div class="summary-core-stats">
                    <div class="summary-stat-box">
                        <label>{move_tr!("armor-class")}</label>
                        <span>{move || combat.armor_class().get()}</span>
                    </div>
                    <div class="summary-stat-box">
                        <label>{move_tr!("initiative")}</label>
                        <span>{move || format_bonus(initiative.get())}</span>
                    </div>
                    <div class="summary-stat-box">
                        <label>{move_tr!("speed")}</label>
                        <span>{move || combat.speed().get()}</span>
                    </div>
                </div>

                // -- Ability modifiers --
                <h4 class="summary-subsection-title">{move_tr!("summary-ability-mods")}</h4>
                <div class="summary-abilities-grid">
                    {Ability::iter().map(|ability| {
                        let tr_key = ability.tr_abbr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        view! {
                            <div class="summary-ability">
                                <span class="summary-ability-label">{label}</span>
                                <span class="summary-ability-mod">{move || {
                                    let modifier = store.read().ability_modifier(ability);
                                    format_bonus(modifier)
                                }}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>

                // -- Saving throws --
                <h4 class="summary-subsection-title">{move_tr!("summary-saving-throws")}</h4>
                <div class="summary-saves-grid">
                    {Ability::iter().map(|ability| {
                        let tr_key = ability.tr_abbr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        view! {
                            <div class="summary-save" class:proficient=move || store.read().proficient_with(ability)>
                                <span class="summary-save-label">{label}</span>
                                <span class="summary-save-value">{move || {
                                    let bonus = store.read().saving_throw_bonus(ability);
                                    format_bonus(bonus)
                                }}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>

                // -- Skills --
                <h4 class="summary-subsection-title">{move_tr!("panel-skills")}</h4>
                <div class="summary-saves-grid">
                    {Skill::iter().map(|skill| {
                        let tr_key = skill.tr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        let proficient = move || store.read().skill_proficiency(skill).is_proficient();
                        view! {
                            <div class="summary-save" class:proficient=proficient>
                                <span class="summary-save-label">{label}</span>
                                <span class="summary-save-value">{move || {
                                    let bonus = store.read().saving_throw_bonus(skill.ability());
                                    format_bonus(bonus)
                                }}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </div>
        }
    }
}

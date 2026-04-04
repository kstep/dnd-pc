use leptos::{html::Input, prelude::*};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::icon::Icon,
    effective::{AdvantageState, EffectiveCharacter},
    model::{
        Ability, Character, CharacterStoreFields, CombatStatsStoreFields, DamageType, Skill,
        Translatable, format_bonus,
    },
};

pub fn adv_icon(state: AdvantageState) -> impl IntoView {
    match state {
        AdvantageState::Advantage => Some(view! {
            <span class="adv-up"><Icon name="chevron-up" size=14 /></span>
        }),
        AdvantageState::Disadvantage => Some(view! {
            <span class="adv-down"><Icon name="chevron-down" size=14 /></span>
        }),
        AdvantageState::Flat => None,
    }
}

#[component]
pub fn StatsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let i18n = expect_context::<I18n>();

    let combat = store.combat();
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

    let show_damage_picker = RwSignal::new(false);

    let apply_damage = move |damage_type: Option<DamageType>| {
        let damage = damage_value();
        if damage > 0 {
            let modified = match damage_type {
                Some(dt) => {
                    let modifiers = eff.damage_modifiers();
                    modifiers.get(&dt).map_or(damage, |m| m.modify(damage))
                }
                None => damage,
            };
            combat.update(|combat| combat.damage(modified));
        }
        show_damage_picker.set(false);
    };

    move || {
        let modifiers = eff.damage_modifiers();
        let has_modifiers = !modifiers.is_empty();

        view! {
            <div class="session-section session-section-stats" id="session-stats">
                <h3 class="session-section-title">{move_tr!("session-stats")}</h3>

                // -- HP --
                <div class="session-core-stats">
                    <div class="session-stat-box session-hp-box">
                        <label>{move_tr!("hp")}</label>
                        <div class="session-hp-controls">
                            <div class="session-hp-damage">
                                <input type="number" min="1" required class="session-damage-input" node_ref=damage_input />
                                <div class="btn-container">
                                    <button class="btn-icon btn-icon--danger" title=move_tr!("damage")
                                        on:click=move |_| {
                                            if has_modifiers {
                                                show_damage_picker.update(|v| *v = !*v);
                                            } else {
                                                apply_damage(None);
                                            }
                                        }
                                    ><Icon name="swords" size=14 /></button>
                                    <button class="btn-icon btn-icon--success" title=move_tr!("heal")
                                        on:click=move |_| {
                                            let heal = damage_value();
                                            if heal > 0 {
                                                combat.update(|combat| combat.heal(heal));
                                            }
                                        }
                                    ><Icon name="heart-plus" size=14 /></button>
                                </div>
                                <Show when=move || show_damage_picker.get()>
                                    <div class="cast-slot-picker">
                                        <button class="cast-slot-pill natural-level"
                                            title=move_tr!("damage")
                                            on:click=move |_| apply_damage(None)
                                        ><Icon name="swords" size=14 /></button>
                                        {modifiers.keys().map(|&damage_type| {
                                            let tr_key = damage_type.tr_key();
                                            let title = Signal::derive(move || i18n.tr(tr_key));
                                            view! {
                                                <button class="cast-slot-pill"
                                                    title=title
                                                    on:click=move |_| apply_damage(Some(damage_type))
                                                ><Icon name=damage_type.icon_name() size=14 /></button>
                                            }
                                        }).collect_view()}
                                        <button class="btn-icon"
                                            on:click=move |_| show_damage_picker.set(false)
                                        ><Icon name="x" size=14 /></button>
                                    </div>
                                </Show>
                            </div>
                            <div class="session-hp-value">
                                {move || combat.hp_current().get()}
                                " ("
                                <input type="number" min="0" class="session-hp-temp-input" prop:value=move || combat.hp_temp().get()
                                    on:change=move |event| {
                                        let value = event_target_value(&event).parse().unwrap_or_default();
                                        combat.hp_temp().set(value);
                                    }
                                />
                                ")"
                                <span class="session-hp-max">
                                    "/ " {move || eff.hp_max()}
                                </span>
                            </div>
                        </div>
                    </div>
                    // -- Inspiration toggle --
                    <div class="session-stat-box session-inspiration-box">
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
                    <div class="session-death-saves">
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
                <div class="session-core-stats">
                    <div class="session-stat-box">
                        <label>{move_tr!("armor-class")}</label>
                        <span>{move || eff.armor_class()}</span>
                    </div>
                    <div class="session-stat-box">
                        <label>{move_tr!("initiative")}</label>
                        <span>{move || format_bonus(eff.initiative())}</span>
                    </div>
                    <div class="session-stat-box">
                        <label>{move_tr!("speed")}</label>
                        <span>{move || eff.speed()}</span>
                    </div>
                </div>

                // -- Ability modifiers --
                <h4 class="session-subsection-title">{move_tr!("session-ability-mods")}</h4>
                <div class="session-abilities-grid">
                    {Ability::iter().map(|ability| {
                        let tr_key = ability.tr_abbr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        view! {
                            <div class="session-ability">
                                <span class="session-ability-label">{label}</span>
                                <span class="session-ability-mod">
                                    {move || format_bonus(eff.ability_modifier(ability))}
                                    {move || adv_icon(eff.ability_advantage(ability))}
                                </span>
                            </div>
                        }
                    }).collect_view()}
                </div>

                // -- Saving throws --
                <h4 class="session-subsection-title">{move_tr!("session-saving-throws")}</h4>
                <div class="session-saves-grid">
                    {Ability::iter().map(|ability| {
                        let tr_key = ability.tr_abbr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        view! {
                            <div class="session-save" class:proficient=move || store.read().proficient_with(ability)>
                                <span class="session-save-label">{label}</span>
                                <span class="session-save-value">
                                    {move || format_bonus(eff.saving_throw_bonus(ability))}
                                    {move || adv_icon(eff.save_advantage(ability))}
                                </span>
                            </div>
                        }
                    }).collect_view()}
                </div>

                // -- Skills --
                <h4 class="session-subsection-title">{move_tr!("panel-skills")}</h4>
                <div class="session-saves-grid">
                    {Skill::iter().map(|skill| {
                        let tr_key = skill.tr_key();
                        let label = Signal::derive(move || i18n.tr(tr_key));
                        let proficient = move || store.read().skill_proficiency(skill).is_proficient();
                        view! {
                            <div class="session-save" class:proficient=proficient>
                                <span class="session-save-label">{label}</span>
                                <span class="session-save-value">
                                    {move || format_bonus(eff.skill_bonus(skill))}
                                    {move || adv_icon(eff.skill_advantage(skill))}
                                </span>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </div>
        }
    }
}

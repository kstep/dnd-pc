use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{icon::Icon, panel::Panel},
    model::{Ability, Character, CharacterStoreFields, DamageType, Translatable, format_bonus},
};

#[component]
pub fn SavingThrowsPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let i18n = expect_context::<leptos_fluent::I18n>();

    view! {
        <Panel title=move_tr!("panel-saving-throws") class="saving-throws-panel">
            {Ability::iter()
                .map(|ability| {
                    let proficient = Memo::new(move |_| {
                        store.saving_throws().read().contains(&ability)
                    });
                    let bonus = Memo::new(move |_| store.read().saving_throw_bonus(ability));
                    let bonus_display = move || format_bonus(bonus.get());
                    let tr_key = ability.tr_key();
                    let label = Signal::derive(move || i18n.tr(tr_key));

                    view! {
                        <div class="save-row">
                            <button
                                class="prof-toggle"
                                on:click=move |_| {
                                    store.saving_throws().update(|st| {
                                        if !st.remove(&ability) {
                                            st.insert(ability);
                                        }
                                    });
                                }
                            >
                                {move || if proficient.get() { "\u{25CF}" } else { "\u{25CB}" }}
                            </button>
                            <span class="save-bonus">{bonus_display}</span>
                            <span class="save-label">{label}</span>
                        </div>
                    }
                })
                .collect_view()}

            // --- Resistances ---
            <h4 class="panel-subsection-title">{move_tr!("summary-resistances")}</h4>
            {DamageType::iter()
                .map(|dt| {
                    let mods = Memo::new(move |_| {
                        store
                            .resistances()
                            .read()
                            .get(&dt)
                            .copied()
                            .unwrap_or_default()
                    });
                    let tr_key = dt.tr_key();
                    let label = Signal::derive(move || i18n.tr(tr_key));
                    let icon = dt.icon_name();

                    view! {
                        <div class="damage-row">
                            <span class="damage-dt-icon"><Icon name=icon size=14 /></span>
                            <span class="damage-label">{label}</span>
                            <button
                                class=move || if mods.get().resistant { "damage-toggle active" } else { "damage-toggle" }
                                title=move || i18n.tr("damage-resistance")
                                on:click=move |_| {
                                    store.resistances().update(|resistances| {
                                        let entry = resistances.entry(dt).or_default();
                                        entry.resistant = !entry.resistant;
                                        if !entry.is_active() {
                                            resistances.remove(&dt);
                                        }
                                    });
                                }
                            >
                                <Icon name="shield-half" size=14 />
                            </button>
                            <button
                                class=move || if mods.get().vulnerable { "damage-toggle active" } else { "damage-toggle" }
                                title=move || i18n.tr("damage-vulnerability")
                                on:click=move |_| {
                                    store.resistances().update(|resistances| {
                                        let entry = resistances.entry(dt).or_default();
                                        entry.vulnerable = !entry.vulnerable;
                                        if !entry.is_active() {
                                            resistances.remove(&dt);
                                        }
                                    });
                                }
                            >
                                <Icon name="shield-off" size=14 />
                            </button>
                            <button
                                class=move || if mods.get().immune { "damage-toggle active" } else { "damage-toggle" }
                                title=move || i18n.tr("damage-immunity")
                                on:click=move |_| {
                                    store.resistances().update(|resistances| {
                                        let entry = resistances.entry(dt).or_default();
                                        entry.immune = !entry.immune;
                                        if !entry.is_active() {
                                            resistances.remove(&dt);
                                        }
                                    });
                                }
                            >
                                <Icon name="shield-check" size=14 />
                            </button>
                            <span class="damage-dr">
                                <Icon name="shield-minus" size=14 />
                                <input
                                    type="number"
                                    min="0"
                                    class="damage-dr-input"
                                    prop:value=move || mods.get().reduction
                                    on:input=move |event| {
                                        let value = event_target_value(&event)
                                            .parse::<u32>()
                                            .unwrap_or(0);
                                        store.resistances().update(|resistances| {
                                            let entry = resistances.entry(dt).or_default();
                                            entry.reduction = value;
                                            if !entry.is_active() {
                                                resistances.remove(&dt);
                                            }
                                        });
                                    }
                                />
                            </span>
                        </div>
                    }
                })
                .collect_view()}
        </Panel>
    }
}

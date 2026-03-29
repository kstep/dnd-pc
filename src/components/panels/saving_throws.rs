use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::panel::Panel,
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
                    let level = Memo::new(move |_| {
                        store
                            .resistances()
                            .read()
                            .get(&dt)
                            .copied()
                            .unwrap_or_default()
                    });
                    let tr_key = dt.tr_key();
                    let label = Signal::derive(move || i18n.tr(tr_key));

                    view! {
                        <div class="save-row">
                            <button
                                class="prof-toggle"
                                on:click=move |_| {
                                    store.resistances().update(|resistances| {
                                        let next = level.get_untracked().next();
                                        if next.is_active() {
                                            resistances.insert(dt, next);
                                        } else {
                                            resistances.remove(&dt);
                                        }
                                    });
                                }
                            >
                                {move || level.get().symbol()}
                            </button>
                            <span class="save-label">{label}</span>
                            {move || {
                                let current = level.get();
                                if current.is_active() {
                                    let level_key = current.tr_key();
                                    Some(view! {
                                        <span class="resistance-level">{Signal::derive(move || i18n.tr(level_key))}</span>
                                    })
                                } else {
                                    None
                                }
                            }}
                        </div>
                    }
                })
                .collect_view()}
        </Panel>
    }
}

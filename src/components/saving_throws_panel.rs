use leptos::prelude::*;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::panel::Panel,
    model::{Ability, Character, CharacterStoreFields},
};

#[component]
pub fn SavingThrowsPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    view! {
        <Panel title="Saving Throws" class="saving-throws-panel">
            {Ability::iter()
                .map(|ability| {
                    let proficient = Memo::new(move |_| {
                        store.saving_throws().read().get(&ability).copied().unwrap_or(false)
                    });
                    let bonus = Memo::new(move |_| store.get().saving_throw_bonus(ability));
                    let bonus_display = move || {
                        let b = bonus.get();
                        if b >= 0 { format!("+{b}") } else { format!("{b}") }
                    };

                    view! {
                        <div class="save-row">
                            <button
                                class="prof-toggle"
                                on:click=move |_| {
                                    store.saving_throws().update(|st| {
                                        let entry = st.entry(ability).or_insert(false);
                                        *entry = !*entry;
                                    });
                                }
                            >
                                {move || if proficient.get() { "\u{25CF}" } else { "\u{25CB}" }}
                            </button>
                            <span class="save-bonus">{bonus_display}</span>
                            <span class="save-label">{ability.to_string()}</span>
                        </div>
                    }
                })
                .collect_view()}
        </Panel>
    }
}

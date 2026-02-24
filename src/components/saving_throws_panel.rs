use leptos::prelude::*;
use strum::IntoEnumIterator;

use crate::model::{Ability, Character};

#[component]
pub fn SavingThrowsPanel() -> impl IntoView {
    let char_signal = use_context::<RwSignal<Character>>().expect("Character context");

    view! {
        <div class="panel saving-throws-panel">
            <h3>"Saving Throws"</h3>
            {Ability::iter()
                .map(|ability| {
                    let proficient = Memo::new(move |_| {
                        char_signal.get().saving_throws.get(&ability).copied().unwrap_or(false)
                    });
                    let bonus = Memo::new(move |_| char_signal.get().saving_throw_bonus(ability));
                    let bonus_display = move || {
                        let b = bonus.get();
                        if b >= 0 { format!("+{b}") } else { format!("{b}") }
                    };

                    view! {
                        <div class="save-row">
                            <button
                                class="prof-toggle"
                                on:click=move |_| {
                                    char_signal.update(|c| {
                                        let entry = c.saving_throws.entry(ability).or_insert(false);
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
        </div>
    }
}

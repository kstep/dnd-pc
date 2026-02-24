use leptos::prelude::*;

use crate::model::{Ability, Character};

#[component]
pub fn AbilityScoreBlock(ability: Ability) -> impl IntoView {
    let char_signal = use_context::<RwSignal<Character>>().expect("Character context");

    let score = Memo::new(move |_| char_signal.get().abilities.get(ability));
    let modifier = Memo::new(move |_| char_signal.get().ability_modifier(ability));

    let modifier_display = move || {
        let m = modifier.get();
        if m >= 0 {
            format!("+{m}")
        } else {
            format!("{m}")
        }
    };

    view! {
        <div class="ability-block">
            <span class="ability-label">{ability.to_string()}</span>
            <span class="ability-modifier">{modifier_display}</span>
            <input
                type="number"
                class="ability-score"
                min="1"
                max="30"
                prop:value=move || score.get().to_string()
                on:input=move |e| {
                    if let Ok(v) = event_target_value(&e).parse::<u32>() {
                        char_signal.update(|c| c.abilities.set(ability, v.clamp(1, 30)));
                    }
                }
            />
        </div>
    }
}

use leptos::prelude::*;
use reactive_stores::Store;

use crate::model::{Ability, Character, CharacterStoreFields, Translatable, format_bonus};

#[component]
pub fn AbilityScoreBlock(ability: Ability) -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let score = Memo::new(move |_| store.read().abilities.get(ability));
    let modifier = Memo::new(move |_| store.read().ability_modifier(ability));

    let modifier_display = move || format_bonus(modifier.get());

    let tr_key = ability.tr_key();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let label = Signal::derive(move || i18n.tr(tr_key));

    view! {
        <div class="ability-block">
            <span class="ability-label">{label}</span>
            <span class="ability-modifier">{modifier_display}</span>
            <input
                type="number"
                class="ability-score"
                min="1"
                max="30"
                prop:value=move || score.get().to_string()
                on:input=move |e| {
                    if let Ok(value) = event_target_value(&e).parse::<u32>() {
                        store.abilities().write().set(ability, value.clamp(1, 30));
                    }
                }
            />
        </div>
    }
}

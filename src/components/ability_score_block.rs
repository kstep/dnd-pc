use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::stat_box::StatBox,
    model::{Ability, Character, CharacterStoreFields, Translatable, format_bonus},
};

#[component]
pub fn AbilityScoreBlock(ability: Ability) -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let score = Memo::new(move |_| store.read().ability_score(ability));
    let modifier = Memo::new(move |_| store.read().ability_modifier(ability));
    let modifier_display = move || format_bonus(modifier.get());

    let tr_key = ability.tr_abbr_key();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let label = Signal::derive(move || i18n.tr(tr_key));

    view! {
        <StatBox label=label>
            <span class="stat-highlight">{modifier_display}</span>
            <input
                type="number"
                min="1"
                max="30"
                prop:value=move || score.get().to_string()
                on:input=move |e| {
                    if let Ok(value) = event_target_value(&e).parse::<u32>() {
                        store.abilities().write().set(ability, value.clamp(1, 30));
                    }
                }
            />
        </StatBox>
    }
}

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::{icon::Icon, stat_box::StatBox},
    model::{Ability, Character, CharacterStoreFields, format_bonus},
};

#[component]
pub fn SaveRow(ability: Ability) -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let proficient = Memo::new(move |_| store.saving_throws().read().contains(&ability));
    let bonus = Memo::new(move |_| store.read().saving_throw_bonus(ability));
    let bonus_display = move || format_bonus(bonus.get());

    let i18n = expect_context::<leptos_fluent::I18n>();
    let label = Signal::derive(move || i18n.tr("saving-throw"));

    view! {
        <StatBox
            label=label
            highlighted=proficient
            class:clickable=true
            on:click=move |_| {
                store.saving_throws().update(|saves| saves.toggle(ability));
            }
        >
            <span class="stat-highlight">{bonus_display}</span>
            <Icon
                name=Signal::derive(move || if proficient.get() { "circle-dot" } else { "circle-dashed" })
            />
        </StatBox>
    }
}

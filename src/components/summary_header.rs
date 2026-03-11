use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::components::A;
use reactive_stores::Store;

use crate::{
    BASE_URL,
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields},
    rules::RulesRegistry,
};

#[component]
pub fn SummaryHeader() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let char_id = store.read_untracked().id;

    let name = Memo::new(move |_| store.identity().name().get());

    let race_display = Memo::new(move |_| {
        let race_name = store.identity().race().get();
        if race_name.is_empty() {
            return String::new();
        }
        registry
            .with_race_entries(|entries| {
                entries
                    .get(race_name.as_str())
                    .map(|e| e.label().to_string())
            })
            .unwrap_or(race_name)
    });

    let class_summary = Memo::new(move |_| store.read().class_summary());
    let total_level = Memo::new(move |_| store.read().level());
    let prof_bonus = Memo::new(move |_| store.read().proficiency_bonus());

    view! {
        <div class="panel summary-header">
            <div class="summary-header-info">
                <span class="summary-header-name">{name}</span>
                <span class="summary-header-detail">{race_display}</span>
                <span class="summary-header-detail">{class_summary}</span>
                <span class="summary-header-stat">
                    {move_tr!("total-level")} ": " <strong>{total_level}</strong>
                </span>
                <span class="summary-header-stat">
                    {move_tr!("prof-bonus")} ": +" <strong>{prof_bonus}</strong>
                </span>
            </div>
            <div class="nav-links">
                <A href=format!("{BASE_URL}/") attr:class="back-link">{move_tr!("back-to-characters")}</A>
                <A href=format!("{BASE_URL}/c/{char_id}") attr:class="back-link">
                    {move_tr!("view-full-sheet")}
                </A>
            </div>
        </div>
    }
}

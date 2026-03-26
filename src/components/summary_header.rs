use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    effective::EffectiveCharacter,
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields},
    rules::RulesRegistry,
};

#[component]
pub fn SummaryHeader() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let registry = expect_context::<RulesRegistry>();

    let name = Memo::new(move |_| store.identity().name().get());

    let species_display = Memo::new(move |_| {
        let species_name = store.identity().species().get();
        if species_name.is_empty() {
            return String::new();
        }
        registry
            .with_species_entries(|entries| {
                entries
                    .get(species_name.as_str())
                    .map(|e| e.label().to_string())
            })
            .unwrap_or(species_name)
    });

    let class_summary = Memo::new(move |_| store.read().class_summary());
    let total_level = Memo::new(move |_| store.read().level());
    let prof_bonus = Memo::new(move |_| eff.proficiency_bonus());

    view! {
        <div class="panel summary-header">
            <div class="summary-header-info">
                <span class="summary-header-name">{name}</span>
                <span class="summary-header-detail">{species_display}</span>
                <span class="summary-header-detail">{class_summary}</span>
                <span class="summary-header-stat">
                    {move_tr!("total-level")} ": " <strong>{total_level}</strong>
                </span>
                <span class="summary-header-stat">
                    {move_tr!("prof-bonus")} ": +" <strong>{prof_bonus}</strong>
                </span>
            </div>
        </div>
    }
}

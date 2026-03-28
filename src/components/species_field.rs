use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::entity_field::EntityField,
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields},
    rules::{DefinitionStore, RulesRegistry},
};

/// Species name selector. Sets `identity.species` and triggers definition
/// fetch. No apply button — wrap in `ApplyFieldSection` for that.
#[component]
pub fn SpeciesField() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let options = Memo::new(move |_| {
        registry.with_species_entries(|entries| {
            entries
                .values()
                .map(|entry| {
                    (
                        entry.name.clone(),
                        entry.label().to_string(),
                        entry.description.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });

    view! {
        <EntityField
            name=move || store.identity().species().get()
            options=options
            ref_prefix="species"
            placeholder=move_tr!("species")
            on_input=move |name: String| {
                let old = store.identity().species().get_untracked();
                store.identity().species().set(name.clone());
                if name != old {
                    store.identity().species_applied().set(false);
                }
                registry.species().fetch(&name);
            }
        />
    }
}

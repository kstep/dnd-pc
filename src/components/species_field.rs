use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{datalist::DatalistOption, entity_field::EntityField},
    model::{AppliedStoreFields, Character, CharacterIdentityStoreFields, CharacterStoreFields},
    rules::{DefinitionStore, IndexEntry, RulesRegistry},
};

/// Species name selector. Sets `identity.species` and triggers definition
/// fetch. Identity changes apply via the header's Rebuild action.
#[component]
pub fn SpeciesField() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let options = Memo::new(move |_| {
        registry.with_species_entries(|entries| {
            entries
                .values()
                .map(|entry| {
                    let (label, description) = registry
                        .index()
                        .entry_label_desc(IndexEntry::Species(&entry.name));
                    DatalistOption::with_signals(&*entry.name, label, description)
                })
                .collect::<Vec<_>>()
        })
    });

    view! {
        <EntityField
            name=move || store.identity().species().get()
            options=options
            kind=|n| IndexEntry::Species(n)
            required=true
            placeholder=move_tr!("species")
            on_input=move |name: String| {
                let old = store.identity().species().get_untracked();
                store.identity().species().set(name.clone());
                if name != old {
                    store.applied().species().set(false);
                }
                registry.species().fetch_untracked(&name);
            }
        />
    }
}

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{datalist::DatalistOption, entity_field::EntityField},
    model::{AppliedStoreFields, Character, CharacterIdentityStoreFields, CharacterStoreFields},
    rules::{DefinitionStore, IndexEntry, RulesRegistry},
};

/// Background name selector. Sets `identity.background` and triggers
/// definition fetch. Identity changes apply via the header's Rebuild action.
#[component]
pub fn BackgroundField() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let options = Memo::new(move |_| {
        registry.with_background_entries(|entries| {
            entries
                .values()
                .map(|entry| {
                    let (label, description) = registry
                        .index()
                        .entry_label_desc(IndexEntry::Background(&entry.name));
                    DatalistOption::with_signals(&*entry.name, label, description)
                })
                .collect::<Vec<_>>()
        })
    });

    view! {
        <EntityField
            name=move || store.identity().background().get()
            options=options
            kind=|n| IndexEntry::Background(n)
            required=true
            placeholder=move_tr!("background")
            on_input=move |name: String| {
                let old = store.identity().background().get_untracked();
                store.identity().background().set(name.clone());
                if name != old {
                    store.applied().background().set(false);
                }
                registry.backgrounds().fetch_untracked(&name);
            }
        />
    }
}

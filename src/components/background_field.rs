use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::entity_field::EntityField,
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields},
    rules::{DefinitionStore, RulesRegistry},
};

/// Background name selector. Sets `identity.background` and triggers
/// definition fetch. No apply button — wrap in `ApplyFieldSection` for that.
#[component]
pub fn BackgroundField() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let options = Memo::new(move |_| {
        registry.with_background_entries(|entries| {
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
            name=move || store.identity().background().get()
            options=options
            ref_prefix="background"
            placeholder=move_tr!("background")
            on_input=move |name: String| {
                let old = store.identity().background().get_untracked();
                store.identity().background().set(name.clone());
                if name != old {
                    store.identity().background_applied().set(false);
                }
                registry.backgrounds().fetch(&name);
            }
        />
    }
}

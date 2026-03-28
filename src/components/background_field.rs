use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use super::character_header::apply_with_args_modal;
use crate::{
    components::entity_field::EntityField,
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields},
    rules::{DefinitionStore, RulesRegistry},
};

#[component]
pub fn BackgroundField() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let bg_options = Memo::new(move |_| {
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
            applied=move || store.identity().background_applied().get()
            options=bg_options
            ref_prefix="background"
            apply_title=move_tr!("btn-apply-background")
            placeholder=move_tr!("background")
            on_input=move |name: String| {
                let old = store.identity().background().get_untracked();
                store.identity().background().set(name.clone());
                if name != old {
                    store.identity().background_applied().set(false);
                }
                registry.backgrounds().fetch(&name);
            }
            fetch=move |name: &str| registry.backgrounds().fetch(name)
            has=move |name: &str| registry.backgrounds().has_tracked(name)
            apply=move |_name: &str| {
                let pending = store
                    .with_untracked(|character| {
                        registry
                            .backgrounds()
                            .with(&character.identity.background, |bg_def| {
                                registry.pending_args_for_features(
                                    character,
                                    bg_def.features.iter().map(String::as_str),
                                )
                            })
                    })
                    .unwrap_or_default();
                apply_with_args_modal(pending, move |args_map| {
                    store.update(|character| registry.apply_background(character, args_map));
                });
            }
        />
    }
}

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
pub fn SpeciesField() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let species_options = Memo::new(move |_| {
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
            applied=move || store.identity().species_applied().get()
            options=species_options
            ref_prefix="species"
            apply_title=move_tr!("btn-apply-species")
            placeholder=move_tr!("species")
            on_input=move |name: String| {
                let old = store.identity().species().get_untracked();
                store.identity().species().set(name.clone());
                if name != old {
                    store.identity().species_applied().set(false);
                }
                registry.species().fetch(&name);
            }
            fetch=move |name: &str| registry.species().fetch(name)
            has=move |name: &str| registry.species().has_tracked(name)
            apply=move |_name: &str| {
                let pending = store
                    .with_untracked(|character| {
                        registry
                            .species()
                            .with(&character.identity.species, |species_def| {
                                registry.pending_args_for_features(
                                    character,
                                    species_def.features.iter().map(String::as_str),
                                )
                            })
                    })
                    .unwrap_or_default();
                apply_with_args_modal(pending, move |args_map| {
                    store.update(|character| registry.apply_species(character, args_map));
                });
            }
        />
    }
}

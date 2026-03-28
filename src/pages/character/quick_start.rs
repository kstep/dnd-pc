use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;

use crate::{
    components::{
        background_field::BackgroundField, character_header::apply_with_args_modal,
        class_field::ClassField, species_field::SpeciesField,
    },
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields, Feature},
    rules::{DefinitionStore, RulesRegistry},
};

#[component]
pub fn QuickStart() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let generation_method = RwSignal::new(String::new());

    let generation_options = Memo::new(move |_| {
        registry.with_features_index(|idx| {
            idx.values()
                .filter(|feat| feat.selectable && feat.name.starts_with("Generation:"))
                .map(|feat| (feat.name.clone(), feat.label().to_string()))
                .collect::<Vec<_>>()
        })
    });

    // --- Create: apply generation → species → background → class ---
    let on_create = move |_| {
        apply_generation(store, registry, generation_method);
    };

    let on_skip = move |_| {
        navigate_to_sheet(store);
    };

    view! {
        <div class="quick-start-page">
            <h2>{move_tr!("quick-start-title")}</h2>

            <div class="quick-start-section">
                <label>{move_tr!("character-name")}</label>
                <input
                    type="text"
                    autofocus
                    prop:value=move || store.identity().name().get()
                    on:input=move |event| {
                        store.identity().name().set(event_target_value(&event));
                    }
                />
            </div>

            <div class="quick-start-section">
                <label>{move_tr!("quick-start-generation")}</label>
                <div class="generation-options">
                    <For
                        each=move || generation_options.get()
                        key=|(name, _)| name.clone()
                        let:option
                    >
                        {
                            let name = option.0.clone();
                            let label = option.1.clone();
                            let name_for_check = name.clone();
                            view! {
                                <label class="generation-option">
                                    <input
                                        type="radio"
                                        name="generation"
                                        prop:value=name.clone()
                                        prop:checked=move || {
                                            generation_method.get() == name_for_check
                                        }
                                        on:change={
                                            let name = name.clone();
                                            move |_| generation_method.set(name.clone())
                                        }
                                    />
                                    {label}
                                </label>
                            }
                        }
                    </For>
                </div>
            </div>

            <div class="quick-start-section">
                <label>{move_tr!("species")}</label>
                <SpeciesField />
            </div>

            <div class="quick-start-section">
                <label>{move_tr!("background")}</label>
                <BackgroundField />
            </div>

            <div class="quick-start-section">
                <label>{move_tr!("class")}</label>
                <ClassField />
            </div>

            <div class="quick-start-actions">
                <button class="btn-primary" on:click=on_create>
                    {move_tr!("quick-start-create")}
                </button>
                <button on:click=on_skip>
                    {move_tr!("quick-start-skip")}
                </button>
            </div>
        </div>
    }
}

/// Step 1: apply selected generation feature, then continue to species.
fn apply_generation(
    store: Store<Character>,
    registry: RulesRegistry,
    generation_method: RwSignal<String>,
) {
    let name = generation_method.get_untracked();
    if name.is_empty() {
        apply_species(store, registry);
        return;
    }

    // Check if already applied
    let already_applied = store.with_untracked(|character| {
        character
            .features
            .iter()
            .any(|feature| feature.name == name && feature.applied)
    });
    if already_applied {
        apply_species(store, registry);
        return;
    }

    // Add the feature entry
    store.features().write().push(Feature {
        name: name.clone(),
        ..Feature::default()
    });

    // Apply (may open ArgsModal for Point Buy / Preset)
    if let Some(pending) =
        store.with_untracked(|character| registry.feature_needs_args(character, &name))
    {
        let feat_name = StoredValue::new(name);
        apply_with_args_modal(vec![pending], move |inputs| {
            let name = feat_name.get_value();
            let identity = store.with_untracked(|character| character.identity.clone());
            let level = store.with_untracked(|character| character.level());
            registry.with_feature_source(&identity, &name, |feat_def, source| {
                let args = inputs.and_then(|i| i.args.get(name.as_str()).cloned());
                let dice = inputs.and_then(|i| i.dice.get(name.as_str()).cloned());
                store.update(|character| {
                    feat_def.apply_with_args(level, character, source.as_ref(), args, dice)
                });
            });
            apply_species(store, registry);
        });
    } else {
        let identity = store.with_untracked(|character| character.identity.clone());
        let level = store.with_untracked(|character| character.level());
        registry.with_feature_source(&identity, &name, |feat_def, source| {
            store.update(|character| feat_def.apply(level, character, source.as_ref()));
        });
        apply_species(store, registry);
    }
}

/// Step 2: apply species, then continue to background.
fn apply_species(store: Store<Character>, registry: RulesRegistry) {
    let species = store.identity().species().get_untracked();
    if !species.is_empty()
        && !store.identity().species_applied().get_untracked()
        && registry.species().has(&species)
    {
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
        if !pending.is_empty() {
            apply_with_args_modal(pending, move |inputs| {
                store.update(|character| registry.apply_species(character, inputs));
                apply_background(store, registry);
            });
            return;
        }
        store.update(|character| registry.apply_species(character, None));
    }

    apply_background(store, registry);
}

/// Step 3: apply background, then continue to class.
fn apply_background(store: Store<Character>, registry: RulesRegistry) {
    let background = store.identity().background().get_untracked();
    if !background.is_empty()
        && !store.identity().background_applied().get_untracked()
        && registry.backgrounds().has(&background)
    {
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
        if !pending.is_empty() {
            apply_with_args_modal(pending, move |inputs| {
                store.update(|character| registry.apply_background(character, inputs));
                apply_class_and_navigate(store, registry);
            });
            return;
        }
        store.update(|character| registry.apply_background(character, None));
    }

    apply_class_and_navigate(store, registry);
}

/// Step 4: apply class level 1, then navigate to sheet.
fn apply_class_and_navigate(store: Store<Character>, registry: RulesRegistry) {
    let class_name = store.with_untracked(|character| character.identity.classes[0].class.clone());
    if !class_name.is_empty() && registry.classes().has(&class_name) {
        let has_unapplied = store
            .with_untracked(|character| !character.identity.classes[0].applied_levels.contains(&1));
        if has_unapplied {
            let pending =
                store.with_untracked(|character| registry.features_needing_args(character, 0, 1));
            if !pending.is_empty() {
                apply_with_args_modal(pending, move |inputs| {
                    store.update(|character| {
                        registry.apply_class_level(character, 0, 1, inputs);
                    });
                    navigate_to_sheet(store);
                });
                return;
            }
            store.update(|character| {
                registry.apply_class_level(character, 0, 1, None);
            });
        }
    }

    navigate_to_sheet(store);
}

fn navigate_to_sheet(store: Store<Character>) {
    let id = store.read_untracked().id;
    let navigate = use_navigate();
    navigate(&format!("/c/{id}"), Default::default());
}

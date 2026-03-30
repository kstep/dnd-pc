use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;

use crate::{
    components::{
        background_field::BackgroundField, character_header::apply_modal, class_field::ClassField,
        icon::Icon, species_field::SpeciesField,
    },
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields, Feature},
    names::{self, NamesData},
    rules::{DefinitionStore, FeatureCategory, PendingInputs, RulesRegistry, WhenCondition},
};

#[component]
pub fn QuickStart() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let names_data: LocalResource<Option<NamesData>> = LocalResource::new(names::fetch_names);

    // Auto-fill a random name on load (replacing "New Character")
    Effect::new(move || {
        if let Some(Some(ref data)) = *names_data.read() {
            let current = store.identity().name().get_untracked();
            if current == "New Character" {
                let species = store.identity().species().get_untracked();
                store.identity().name().set(data.generate_name(&species));
            }
        }
    });

    let randomize_name = move |_| {
        if let Some(Some(ref data)) = *names_data.read_untracked() {
            let species = store.identity().species().get_untracked();
            store.identity().name().set(data.generate_name(&species));
        }
    };

    let generation_method = RwSignal::new(String::new());

    let generation_options = Memo::new(move |_| {
        registry.with_features_index(|idx| {
            idx.values()
                .filter(|feat| matches!(feat.category, FeatureCategory::Generation))
                .map(|feat| (feat.name.clone(), feat.label().to_string()))
                .collect::<Vec<_>>()
        })
    });

    let on_create = move |_| {
        create_character(store, registry, generation_method);
    };

    let on_skip = move |_| {
        navigate_to_sheet(store);
    };

    view! {
        <div class="quick-start-page">
            <h2>{move_tr!("quick-start-title")}</h2>

            <div class="quick-start-section">
                <label>{move_tr!("character-name")}</label>
                <div class="entity-input-row">
                    <input
                        type="text"
                        autofocus
                        prop:value=move || store.identity().name().get()
                        on:input=move |event| {
                            store.identity().name().set(event_target_value(&event));
                        }
                    />
                    <button
                        type="button"
                        class="btn-icon"
                        title="Randomize name"
                        on:click=randomize_name
                    >
                        <Icon name="dices" />
                    </button>
                </div>
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

/// Collect all pending inputs from generation + species + background + class
/// into a single modal, then apply everything and navigate to the sheet.
fn create_character(
    store: Store<Character>,
    registry: RulesRegistry,
    generation_method: RwSignal<String>,
) {
    let gen_name = generation_method.get_untracked();

    // Reset all applied state while preserving identity (name, species,
    // background, class selections). Handles cancelled previous attempts.
    store.update(|character| character.clear());

    // Add generation feature entry if selected
    if !gen_name.is_empty() {
        store.features().write().push(Feature {
            name: gen_name.clone(),
            ..Feature::default()
        });
    }

    // Gather ALL pending inputs across all steps into one list
    let mut all_pending: Vec<PendingInputs> = Vec::new();
    store.with_untracked(|character| {
        // Generation feature
        if !gen_name.is_empty()
            && let Some(pending) = registry.feature_needs_args(character, &gen_name)
        {
            all_pending.push(pending);
        }

        // Species features
        let species = &character.identity.species;
        if !species.is_empty()
            && !character.identity.species_applied
            && registry.species().has(species)
        {
            all_pending.extend(
                registry
                    .species()
                    .with(species, |species_def| {
                        registry.pending_args_for_features(
                            character,
                            species_def.features.iter().map(String::as_str),
                        )
                    })
                    .unwrap_or_default(),
            );
        }

        // Background features
        let background = &character.identity.background;
        if !background.is_empty()
            && !character.identity.background_applied
            && registry.backgrounds().has(background)
        {
            all_pending.extend(
                registry
                    .backgrounds()
                    .with(background, |bg_def| {
                        registry.pending_args_for_features(
                            character,
                            bg_def.features.iter().map(String::as_str),
                        )
                    })
                    .unwrap_or_default(),
            );
        }

        // Class level 1 features
        if let Some(class_level) = character.identity.classes.first()
            && !class_level.class.is_empty()
            && registry.classes().has(&class_level.class)
            && !class_level.applied_levels.contains(&1)
        {
            all_pending.extend(registry.features_needing_args(character, 0, 1));
        }
    });

    let gen_name = StoredValue::new(gen_name);

    // Single modal for everything, then apply all steps and navigate
    apply_modal(all_pending, move |inputs| {
        let gen_name = gen_name.get_value();

        // Apply generation feature
        if !gen_name.is_empty() {
            let level = store.with_untracked(|character| character.level());
            registry.with_feature(&gen_name, |feat_def| {
                let feature_inputs = inputs.map(|i| i.get(gen_name.as_str())).unwrap_or_default();
                store.update(|character| {
                    feat_def.apply(
                        level,
                        character,
                        WhenCondition::OnFeatureAdd,
                        feature_inputs,
                    );
                });
            });
        }

        // Apply species
        if !store.identity().species().get_untracked().is_empty()
            && !store.identity().species_applied().get_untracked()
        {
            store.update(|character| registry.apply_species(character, inputs));
        }

        // Apply background
        if !store.identity().background().get_untracked().is_empty()
            && !store.identity().background_applied().get_untracked()
        {
            store.update(|character| registry.apply_background(character, inputs));
        }

        // Apply class level 1
        let has_unapplied_class = store.with_untracked(|character| {
            character
                .identity
                .classes
                .first()
                .is_some_and(|class_level| {
                    !class_level.class.is_empty()
                        && registry.classes().has(&class_level.class)
                        && !class_level.applied_levels.contains(&1)
                })
        });
        if has_unapplied_class {
            store.update(|character| {
                registry.apply_class_level(character, 0, 1, inputs);
            });
        }

        navigate_to_sheet(store);
    });
}

fn navigate_to_sheet(store: Store<Character>) {
    let id = store.read_untracked().id;
    let navigate = use_navigate();
    navigate(&format!("/c/{id}"), Default::default());
}

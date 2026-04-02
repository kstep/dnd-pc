use std::time::Duration;

use leptos::{leptos_dom::helpers::set_timeout, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    components::{
        background_field::BackgroundField, character_header::apply_with_modal,
        class_field::ClassField, icon::Icon, species_field::SpeciesField,
    },
    model::{
        Character, CharacterIdentityStoreFields, CharacterStoreFields, Feature, FeatureSource,
    },
    names::{self, NamesData},
    rules::{
        DefinitionStore, FeatureCategory, RulesRegistry,
        apply::{PendingFeature, apply_new_features, collect_pending_features},
    },
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
        navigate_to_sheet(store.read_untracked().id);
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

    // Collect ALL pending features across all steps into one list
    let all_pending: Vec<PendingFeature> = store.with_untracked(|character| {
        let gen_pending = (!gen_name.is_empty()).then(|| {
            let level = character.level().max(1);
            PendingFeature {
                name: gen_name.clone(),
                source: FeatureSource::User(level),
                level,
            }
        });

        registry.with_features_index_untracked(|fi| {
            let mut pending = collect_pending_features(character, &registry, fi);
            pending.extend(gen_pending);
            pending
        })
    });

    // Single modal for everything, then apply all steps and navigate
    apply_with_modal(
        store,
        registry,
        all_pending,
        move |character, pending, inputs, fi| {
            // Set applied flags
            if !character.identity.species.is_empty() && !character.identity.species_applied {
                character.identity.species_applied = true;
            }
            if !character.identity.background.is_empty() && !character.identity.background_applied {
                character.identity.background_applied = true;
            }
            let class_cache = registry.classes().cache().read_untracked();
            for class_level in &mut character.identity.classes {
                for lvl in 1..=class_level.level {
                    class_level.applied_levels.insert(lvl);
                }
                if let Some(def) = class_cache.get(class_level.class.as_str()) {
                    class_level.hit_die_sides = def.hit_die;
                }
            }

            // Apply all collected features in one pass
            apply_new_features(fi, character, pending, Some(inputs));
            character.combat.hp_current = character.hp_max();

            navigate_to_sheet(character.id);
        },
    );
}

fn navigate_to_sheet(id: Uuid) {
    let navigate = use_navigate();
    set_timeout(
        move || navigate(&format!("/c/{id}"), Default::default()),
        Duration::ZERO,
    );
}

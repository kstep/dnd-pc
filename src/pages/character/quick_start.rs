use std::{collections::BTreeMap, time::Duration};

use leptos::{leptos_dom::helpers::set_timeout, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    BASE_URL,
    components::{
        ai_generate_modal::{AiGenerateModal, AiGenerateResult},
        apply::{apply_with_modal, apply_with_prefilled_args},
        background_field::BackgroundField,
        class_field::ClassField,
        icon::Icon,
        species_field::SpeciesField,
    },
    model::{
        Character, CharacterIdentityStoreFields, CharacterStoreFields, FeatureCategory,
        FeatureSource, PersonalityStoreFields,
    },
    names::{self, NamesData},
    rules::{
        ApplyInputs, DefinitionStore, RulesRegistry,
        apply::{PendingFeature, apply_new_features, collect_pending_features},
        feature::FeatureDefinition,
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

    let generation_options = Memo::new(move |_| {
        registry.with_features_index(|idx| {
            idx.values()
                .filter(|feat| matches!(feat.category, FeatureCategory::Generation))
                .map(|feat| (feat.name.clone(), feat.label().to_string()))
                .collect::<Vec<_>>()
        })
    });

    let generation_method = RwSignal::new(
        generation_options
            .get_untracked()
            .first()
            .map(|(name, _)| name.to_string())
            .unwrap_or_default(),
    );

    let on_create = move |_| {
        create_character(store, registry, generation_method);
    };

    let show_ai_modal = RwSignal::new(false);

    // AI result callback runs synchronously in Effect context — full Leptos
    // Owner available (navigate, ArgsModalCtx, etc.)
    let on_ai_result = Callback::new(move |result: AiGenerateResult| {
        apply_ai_result(store, registry, generation_method, result);
    });

    let skip_href = format!("{BASE_URL}/c/{}", store.read_untracked().id);

    view! {
        <AiGenerateModal show=show_ai_modal on_result=on_ai_result />
        <form class="quick-start-page" on:submit=move |event| {
            event.prevent_default();
            on_create(event);
        }>
            <div class="quick-start-header">
                <h2>{move_tr!("quick-start-title")}</h2>
                <button
                    type="button"
                    class="btn-primary"
                    on:click=move |_| show_ai_modal.set(true)
                >
                    <Icon name="sparkles" size=16 />
                    " "
                    {move_tr!("ai-generate-button")}
                </button>
            </div>

            <div class="quick-start-section">
                <label>{move_tr!("character-name")}</label>
                <div class="entity-input-row">
                    <input
                        type="text"
                        required
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
                        <Icon name="dices" size=16 />
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
                <button type="submit" class="btn-primary">
                    {move_tr!("quick-start-create")}
                </button>
                <a class="btn-link" href=skip_href>
                    {move_tr!("quick-start-skip")}
                </a>
            </div>
        </form>
    }
}

// --- Shared helpers ---

/// Collect all pending features for quick-start creation: generation feature
/// (if selected) + species/background/class features.
fn collect_quick_start_pending(
    store: &Store<Character>,
    registry: &RulesRegistry,
    gen_name: &str,
) -> Vec<PendingFeature> {
    store.with_untracked(|character| {
        let gen_pending = (!gen_name.is_empty()).then(|| {
            let level = character.level().max(1);
            PendingFeature {
                name: gen_name.to_string(),
                source: FeatureSource::User(0),
                level,
            }
        });

        registry.with_features_index_untracked(|fi| {
            let mut pending: Vec<PendingFeature> = gen_pending.into_iter().collect();
            pending.extend(collect_pending_features(character, registry, fi));
            pending
        })
    })
}

/// Apply callback shared by manual and AI quick-start: set applied flags,
/// hit dice, apply features, set HP, navigate to editor.
fn finalize_quick_start(
    registry: RulesRegistry,
    character: &mut Character,
    pending: &[PendingFeature],
    inputs: &ApplyInputs,
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
) {
    if !character.identity.species.is_empty() && !character.applied.species {
        character.applied.species = true;
    }
    if !character.identity.background.is_empty() && !character.applied.background {
        character.applied.background = true;
    }
    let class_cache = registry.classes().cache().read_untracked();
    // Collect class updates first to avoid borrowing character.applied while
    // iterating character.identity.classes.
    let class_updates: Vec<(String, u32)> = character
        .identity
        .classes
        .iter()
        .map(|cl| (cl.class.clone(), cl.level))
        .collect();
    for (class_name, level) in &class_updates {
        for lvl in 1..=*level {
            character.applied.mark_level(class_name, lvl);
        }
    }
    for class_level in &mut character.identity.classes {
        if let Some(def) = class_cache.get(class_level.class.as_str()) {
            class_level.hit_die_sides = def.hit_die;
        }
    }

    apply_new_features(
        features_index,
        character,
        pending,
        Some(&inputs.feature_inputs),
    );
    character.combat.hp_current = character.hp_max();

    navigate_to_editor(character.id);
}

fn navigate_to_editor(id: Uuid) {
    let navigate = use_navigate();
    set_timeout(
        move || navigate(&format!("/c/{id}"), Default::default()),
        Duration::ZERO,
    );
}

// --- Manual creation ---

fn create_character(
    store: Store<Character>,
    registry: RulesRegistry,
    generation_method: RwSignal<String>,
) {
    let gen_name = generation_method.get_untracked();

    // Reset all applied state while preserving identity (name, species,
    // background, class selections). Handles cancelled previous attempts.
    store.update(|character| character.clear());

    let all_pending = collect_quick_start_pending(&store, &registry, &gen_name);

    apply_with_modal(
        store,
        registry,
        all_pending,
        move |character, pending, inputs, fi| {
            finalize_quick_start(registry, character, pending, inputs, fi);
        },
    );
}

// --- AI creation ---

fn apply_ai_result(
    store: Store<Character>,
    registry: RulesRegistry,
    generation_method: RwSignal<String>,
    result: AiGenerateResult,
) {
    let concept = result.concept;
    let prefilled = result.feature_choices;

    // Fill personality fields
    store
        .personality()
        .personality_traits()
        .set(concept.personality_traits);
    store.personality().ideals().set(concept.ideals);
    store.personality().bonds().set(concept.bonds);
    store.personality().flaws().set(concept.flaws);
    store.personality().history().set(concept.backstory);

    // Set generation method to preset
    let preset_name = registry
        .with_features_index_untracked(|idx| {
            idx.values()
                .find(|feat| {
                    matches!(feat.category, FeatureCategory::Generation)
                        && feat.name.contains("Preset")
                })
                .map(|feat| feat.name.to_string())
        })
        .unwrap_or_default();
    generation_method.set(preset_name.clone());

    let all_pending = collect_quick_start_pending(&store, &registry, &preset_name);

    apply_with_prefilled_args(
        store,
        registry,
        all_pending,
        prefilled,
        result.replacements,
        move |character, pending, inputs, fi| {
            finalize_quick_start(registry, character, pending, inputs, fi);
        },
    );
}

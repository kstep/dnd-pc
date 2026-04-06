use std::{collections::BTreeMap, time::Duration};

use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    ai::{self, AiSettings, CharacterConcept, PendingArgDescription},
    components::{icon::Icon, modal::Modal, spinner::Spinner},
    hooks,
    model::{Character, CharacterStoreFields, Feature, FeatureCategory, FeatureSource},
    rules::{
        DefinitionStore, RulesRegistry,
        apply::{PendingFeature, collect_pending_features},
    },
    storage,
};

/// Result of the full AI generation pipeline (Phase 1 + Phase 2).
#[derive(Clone)]
pub struct AiGenerateResult {
    pub concept: CharacterConcept,
    pub feature_choices: BTreeMap<String, Vec<i32>>,
}

struct AiGenerateInput {
    description: String,
    settings: AiSettings,
    species_list: String,
    classes_list: String,
    backgrounds_list: String,
    preset_name: String,
}

/// Run both AI phases: generate identity, wait for definitions, generate
/// feature choices. This is the async body of the Action.
#[allow(clippy::too_many_lines)]
async fn run_ai_generation(
    input: AiGenerateInput,
    store: Store<Character>,
    registry: RulesRegistry,
    phase: RwSignal<&'static str>,
) -> Result<AiGenerateResult, ai::AiError> {
    // Phase 1: generate identity
    phase.set("ai-generate-phase-identity");
    let concept = ai::generate_character(
        &input.settings,
        &input.description,
        &input.classes_list,
        &input.species_list,
        &input.backgrounds_list,
    )
    .await?;

    // Fill store identity so ensure_definitions_fetched can trigger fetches
    store.update(|character| {
        character.identity.name = concept.name.clone();
        character.identity.species = concept.species.clone();
        character.identity.background = concept.background.clone();
        if let Some(class_level) = character.identity.classes.first_mut() {
            class_level.class = concept.class.clone();
            class_level.subclass = concept.subclass.clone();
        }
    });

    // Trigger definition fetches
    store.with_untracked(|character| registry.ensure_definitions_fetched(character));

    // Wait for definitions to load
    let class_name = concept.class.clone();
    let species_name = concept.species.clone();
    let background_name = concept.background.clone();

    for _ in 0..50 {
        let all_loaded = (class_name.is_empty() || registry.classes().has(&class_name))
            && (species_name.is_empty() || registry.species().has(&species_name))
            && (background_name.is_empty() || registry.backgrounds().has(&background_name));
        if all_loaded {
            break;
        }
        hooks::sleep(Duration::from_millis(100)).await;
    }

    // Add generation feature to collect it with pending
    if !input.preset_name.is_empty() {
        store.features().write().push(Feature {
            name: input.preset_name.clone(),
            ..Feature::default()
        });
    }

    // Collect pending features
    let all_pending: Vec<PendingFeature> = store.with_untracked(|character| {
        let gen_pending = (!input.preset_name.is_empty()).then(|| {
            let level = character.level().max(1);
            PendingFeature {
                name: input.preset_name.clone(),
                source: FeatureSource::User(0),
                level,
            }
        });
        registry.with_features_index_untracked(|fi| {
            let mut pending: Vec<PendingFeature> = gen_pending.into_iter().collect();
            pending.extend(collect_pending_features(character, &registry, fi));
            pending
        })
    });

    // Build AI arg descriptions
    let pending_inputs: Vec<_> = registry.with_features_index_untracked(|fi| {
        let character = store.read_untracked();
        all_pending
            .iter()
            .filter_map(|pending_feature| {
                let feat_def = fi.get(pending_feature.name.as_str())?;
                pending_feature.pending_inputs(feat_def, &character)
            })
            .collect()
    });
    let arg_descriptions: Vec<PendingArgDescription> =
        store.with_untracked(|character| ai::describe_pending_args(&pending_inputs, character));

    // Pre-fill Generation: Preset from concept abilities
    let mut feature_choices: BTreeMap<String, Vec<i32>> = BTreeMap::new();
    if !input.preset_name.is_empty() {
        feature_choices.insert(input.preset_name, concept.abilities.to_vec());
    }

    // Phase 2: generate feature choices
    if !arg_descriptions.is_empty() {
        phase.set("ai-generate-phase-choices");
        if let Ok(ai_choices) =
            ai::generate_feature_choices(&input.settings, &concept, &arg_descriptions).await
        {
            for (feature_name, args) in ai_choices {
                // Strip "Feature: " prefix if model included it
                let key = feature_name
                    .strip_prefix("Feature: ")
                    .unwrap_or(&feature_name)
                    .to_string();
                feature_choices.entry(key).or_insert(args);
            }
        }
    }

    Ok(AiGenerateResult {
        concept,
        feature_choices,
    })
}

#[component]
pub fn AiGenerateModal(
    show: RwSignal<bool>,
    on_result: Callback<AiGenerateResult>,
) -> impl IntoView {
    let description = RwSignal::new(String::new());
    let error_text = RwSignal::new(Option::<String>::None);
    let phase = RwSignal::new("");

    let has_key = Memo::new(move |_| storage::load_ai_settings().has_api_key());

    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let generate = Action::new_local(move |description: &String| {
        let description = description.clone();
        let settings = storage::load_ai_settings();

        let species_list = registry.with_species_entries(|entries| {
            entries
                .values()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        });

        let backgrounds_list = registry.with_background_entries(|entries| {
            entries
                .values()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        });

        let classes_list = registry.with_class_entries(|entries| {
            entries
                .values()
                .map(|entry| {
                    let subclasses = registry
                        .classes()
                        .with(&entry.name, |def| {
                            def.subclasses
                                .values()
                                .map(|sub| sub.name.as_ref())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default();
                    if subclasses.is_empty() {
                        entry.name.clone()
                    } else {
                        format!("{} (subclasses: {subclasses})", entry.name)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        });

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

        async move {
            run_ai_generation(
                AiGenerateInput {
                    description,
                    settings,
                    species_list,
                    classes_list,
                    backgrounds_list,
                    preset_name,
                },
                store,
                registry,
                phase,
            )
            .await
        }
    });

    // Handle Action result
    Effect::new(move || {
        let value = generate.value().read();
        let Some(ref result) = *value else { return };
        match result {
            Ok(ai_result) => {
                description.set(String::new());
                show.set(false);
                on_result.run(ai_result.clone());
            }
            Err(error) => {
                error_text.set(Some(error.to_string()));
            }
        }
    });

    let generating = generate.pending();

    // Reset state when modal opens
    Effect::new(move || {
        if show.get() {
            error_text.set(None);
            phase.set("");
        }
    });

    let on_generate = move |_| {
        let desc = description.get_untracked();
        if !desc.trim().is_empty() && has_key.get_untracked() {
            generate.dispatch(desc);
        }
    };

    view! {
        <Modal show title=move_tr!("ai-generate-title")>
            <div class="modal-body ai-generate-modal">
                {move || {
                    if !has_key.get() {
                        Either::Left(
                            view! {
                                <p class="ai-generate-no-key">
                                    {move_tr!("ai-generate-no-key")}
                                </p>
                            },
                        )
                    } else {
                        Either::Right(
                            view! {
                                <div class="textarea-field">
                                    <label>{move_tr!("ai-generate-description")}</label>
                                    <textarea
                                        rows="4"
                                        placeholder=move_tr!("ai-generate-placeholder")
                                        prop:value=move || description.get()
                                        on:input=move |event| {
                                            description.set(event_target_value(&event));
                                        }
                                        prop:disabled=move || generating.get()
                                    />
                                </div>

                                {move || {
                                    error_text.get().map(|error| {
                                        view! {
                                            <p class="ai-generate-error">
                                                {move_tr!("ai-generate-error")}
                                                ": "
                                                {error}
                                            </p>
                                        }
                                    })
                                }}

                                <div class="ai-generate-status">
                                    <Spinner loading=Signal::derive(move || generating.get()) />
                                    <Show when=move || generating.get()>
                                        <p class="ai-generate-phase">
                                            {move || {
                                            let i18n = expect_context::<leptos_fluent::I18n>();
                                            i18n.tr(phase.get())
                                        }}
                                        </p>
                                    </Show>
                                </div>
                            },
                        )
                    }
                }}
            </div>
            <div class="modal-actions">
                <Show when=move || has_key.get()>
                    <button
                        type="button"
                        class="btn-primary"
                        prop:disabled=move || {
                            generating.get() || description.get().trim().is_empty()
                        }
                        on:click=on_generate
                    >
                        <Icon name="sparkles" size=16 />
                        " "
                        {move_tr!("ai-generate-button")}
                    </button>
                </Show>
                <button
                    type="button"
                    class="btn-link"
                    on:click=move |_| show.set(false)
                >
                    {move_tr!("import-cancel")}
                </button>
            </div>
        </Modal>
    }
}

use std::{collections::BTreeMap, fmt::Write, time::Duration};

use leptos::{either::Either, prelude::*};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    ai::{self, AiSettings, CharacterConcept, ChatMessage, PendingArgDescription, Role},
    components::{
        ai_settings_modal::AiSettingsModal, apply::ArgsContext, icon::Icon, modal::Modal,
        spinner::Spinner,
    },
    hooks,
    hooks::join_iter,
    model::{Attribute, Character, FeatureCategory, FeatureSource},
    rules::{
        DefinitionStore, PendingInputs, ReplaceWith, RulesRegistry, WhenCondition,
        apply::{PendingFeature, collect_pending_features},
    },
    storage,
};

const DEFINITION_POLL_ATTEMPTS: u32 = 50;
const DEFINITION_POLL_INTERVAL: Duration = Duration::from_millis(100);
const AI_MAX_RETRIES: u32 = 3;

/// Result of the full AI generation pipeline (Phase 1 + Phase 2).
#[derive(Clone)]
pub struct AiGenerateResult {
    pub concept: CharacterConcept,
    pub feature_choices: BTreeMap<String, Vec<i32>>,
    pub replacements: BTreeMap<String, String>,
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
async fn run_ai_generation(
    input: AiGenerateInput,
    store: Store<Character>,
    registry: RulesRegistry,
    phase: RwSignal<&'static str>,
) -> Result<AiGenerateResult, ai::AiError> {
    // Phase 1: generate identity
    phase.set("ai-generate-phase-identity");
    let mut concept = ai::generate_character(
        &input.settings,
        &input.description,
        &input.classes_list,
        &input.species_list,
        &input.backgrounds_list,
    )
    .await?;

    // Canonicalize species/class/background to the registry's English keys.
    // Models occasionally return localized labels even when instructed not to;
    // map them back here. Subclass is canonicalized after the class definition
    // loads (subclass list lives inside ClassDefinition, not the index).
    concept.species = registry
        .canonical_species_name(&concept.species)
        .ok_or_else(|| ai::AiError::Json(format!("unknown species: {:?}", concept.species)))?;
    concept.class = registry
        .canonical_class_name(&concept.class)
        .ok_or_else(|| ai::AiError::Json(format!("unknown class: {:?}", concept.class)))?;
    concept.background = registry
        .canonical_background_name(&concept.background)
        .ok_or_else(|| {
            ai::AiError::Json(format!("unknown background: {:?}", concept.background))
        })?;

    // Fill store identity so ensure_definitions_fetched can trigger fetches.
    // Class is no longer pre-written — the Class Level placeholder + System
    // (Class) feature apply path handles that during the cascade modal.
    store.update(|character| {
        character.personality.name = concept.name.clone();
        character.identity.species = concept.species.clone();
        character.identity.background = concept.background.clone();
    });

    // Trigger definition fetches. Class isn't yet in identity.classes (the
    // Class Level placeholder will set it during apply), so kick its
    // definition fetch separately — the subclass canonicalization below
    // and the cascade's class-feature collection both need it loaded.
    store.with_untracked(|character| registry.ensure_definitions_fetched(character));

    // Wait for definitions to load
    let class_name = concept.class.clone();
    let species_name = concept.species.clone();
    let background_name = concept.background.clone();

    for _ in 0..DEFINITION_POLL_ATTEMPTS {
        let all_loaded = (class_name.is_empty() || registry.classes().has_untracked(&class_name))
            && (species_name.is_empty() || registry.species().has_untracked(&species_name))
            && (background_name.is_empty()
                || registry.backgrounds().has_untracked(&background_name));
        if all_loaded {
            break;
        }
        hooks::sleep(DEFINITION_POLL_INTERVAL).await;
    }

    // Canonicalize subclass against the now-loaded class definition. If the
    // model returned a localized or unknown subclass, drop it rather than
    // poison identity — leaving subclass empty is recoverable; an invalid
    // value is not.
    if let Some(raw_subclass) = concept.subclass.as_deref()
        && !raw_subclass.trim().is_empty()
    {
        let canonical = registry.canonical_subclass_name(&concept.class, raw_subclass);
        concept.subclass = canonical.clone();
        store.update(|character| {
            if let Some(class_level) = character.identity.classes.first_mut() {
                class_level.subclass = canonical.as_deref().map(Box::from);
            }
        });
    }

    // Collect pending features
    let all_pending: Vec<PendingFeature> = store.with_untracked(|character| {
        let gen_pending = (!input.preset_name.is_empty()).then(|| {
            let level = character.level().max(1);
            PendingFeature {
                name: input.preset_name.as_str().into(),
                source: FeatureSource::User(0),
                level,
                replaces: None,
            }
        });
        registry.with_features_index_untracked(|fi| {
            let mut pending: Vec<PendingFeature> = gen_pending.into_iter().collect();
            pending.extend(collect_pending_features(character, &registry, fi));
            pending
        })
    });

    // Build AI arg & replacement descriptions
    let pending_inputs: Vec<_> = registry.with_features_index_untracked(|fi| {
        let character = store.read_untracked();
        all_pending
            .iter()
            .filter_map(|pending_feature| {
                let feat_def = fi.get(&pending_feature.name)?;
                pending_feature.pending_inputs(feat_def, &character)
            })
            .collect()
    });
    let arg_descriptions: Vec<PendingArgDescription> = store.with_untracked(|character| {
        ai::describe_pending_args(&pending_inputs, registry, character)
    });
    let replacement_descriptions = store.with_untracked(|character| {
        ai::describe_pending_replacements(&pending_inputs, registry, character)
    });

    let mut feature_choices: BTreeMap<String, Vec<i32>> = BTreeMap::new();
    let mut replacements: BTreeMap<String, String> = BTreeMap::new();

    // Phase 2: generate feature choices with retry loop
    let has_work = !arg_descriptions.is_empty() || !replacement_descriptions.is_empty();
    if has_work {
        phase.set("ai-generate-phase-choices");
        let mut messages = ai::build_feature_choices_messages(
            &concept,
            &arg_descriptions,
            &replacement_descriptions,
        );

        let (choices, raw_replacements) = send_with_retry(
            &input.settings,
            &mut messages,
            &pending_inputs,
            &store.read_untracked(),
            phase,
        )
        .await;
        feature_choices.extend(choices);

        // Validate replacements against replace_with constraints
        registry.with_features_index_untracked(|fi| {
            for (original, replacement) in &raw_replacements {
                let valid = pending_inputs
                    .iter()
                    .find(|pending_input| &*pending_input.feature_name == original.as_str())
                    .and_then(|pending_input| {
                        fi.get(replacement.as_str())
                            .filter(|feat_def| pending_input.replace_with.matches(feat_def))
                    })
                    .is_some();
                if valid {
                    replacements.insert(original.clone(), replacement.clone());
                }
            }
        });

        // Follow-up: get ARGs for chosen replacements (with retry)
        if !replacements.is_empty() {
            let (replacement_inputs, replacement_arg_descriptions) = registry
                .with_features_index_untracked(|fi| {
                    let character = store.read_untracked();
                    let inputs: Vec<PendingInputs> = replacements
                        .values()
                        .filter_map(|replacement_name| {
                            let feat_def = fi.get(replacement_name.as_str())?;
                            PendingInputs::from_feature(
                                replacement_name.as_str().into(),
                                feat_def,
                                FeatureSource::User(0),
                                WhenCondition::OnFeatureAdd,
                                Vec::new(),
                                ReplaceWith::None,
                            )
                        })
                        .collect();
                    let descriptions = ai::describe_pending_args(&inputs, registry, &character);
                    (inputs, descriptions)
                });

            if !replacement_arg_descriptions.is_empty() {
                phase.set("ai-generate-phase-choices");
                let args_text: String = join_iter(
                    replacement_arg_descriptions.iter().map(|desc| {
                        format!(
                            "- \"{}\"\n  Description: {}\n{}",
                            desc.feature_name, desc.feature_description, desc.args_description
                        )
                    }),
                    "\n\n",
                );

                let mut follow_up_messages = vec![
                    ChatMessage {
                        role: Role::System,
                        content: "You are a D&D 5e character builder. \
                            Provide ARG values for the chosen replacement features. \
                            Respond with valid JSON only, no markdown."
                            .to_string(),
                    },
                    ChatMessage {
                        role: Role::User,
                        content: format!(
                            "Character: {name} ({species} {class})\n\n\
                             Provide ARG values for:\n{args_text}\n\n\
                             Respond with a JSON object: {{ \"Feature Name\": [ARGs], ... }}",
                            name = concept.name,
                            species = concept.species,
                            class = concept.class,
                        ),
                    },
                ];

                let (follow_up_choices, _) = send_with_retry(
                    &input.settings,
                    &mut follow_up_messages,
                    &replacement_inputs,
                    &store.read_untracked(),
                    phase,
                )
                .await;
                feature_choices.extend(follow_up_choices);
            }
        }
    }

    Ok(AiGenerateResult {
        concept,
        feature_choices,
        replacements,
    })
}

/// Run a chat request with retry loop: send messages, merge ARG choices
/// and replacements, validate against guard expressions, retry with
/// feedback on failure.
async fn send_with_retry(
    settings: &AiSettings,
    messages: &mut Vec<ChatMessage>,
    pending_inputs: &[PendingInputs],
    character: &Character,
    phase: RwSignal<&'static str>,
) -> (BTreeMap<String, Vec<i32>>, BTreeMap<String, String>) {
    let mut choices = BTreeMap::new();
    let mut replacements = BTreeMap::new();

    for attempt in 0..AI_MAX_RETRIES {
        let result: Result<(serde_json::Value, String), _> =
            ai::send_chat(settings, messages).await;
        let Ok((value, raw_content)) = result else {
            break;
        };

        let (new_choices, new_replacements) = ai::parse_feature_choices_response(value);
        choices.extend(new_choices);
        replacements.extend(new_replacements);

        messages.push(ChatMessage {
            role: Role::Assistant,
            content: raw_content,
        });

        let errors = validate_feature_choices(&choices, pending_inputs, character);
        if errors.is_empty() {
            break;
        }

        if attempt + 1 < AI_MAX_RETRIES {
            phase.set("ai-generate-phase-retry");
            messages.push(ChatMessage {
                role: Role::User,
                content: format_validation_feedback(&errors),
            });
        }
    }

    (choices, replacements)
}

struct ChoiceValidationError {
    feature_name: String,
    message: String,
}

fn validate_feature_choices(
    choices: &BTreeMap<String, Vec<i32>>,
    pending_inputs: &[PendingInputs],
    character: &Character,
) -> Vec<ChoiceValidationError> {
    let mut errors = Vec::new();
    for pending_input in pending_inputs {
        let Some(args) = choices.get(&*pending_input.feature_name) else {
            errors.push(ChoiceValidationError {
                feature_name: pending_input.feature_name.to_string(),
                message: format!(
                    "missing from response (expected {} ARG values)",
                    pending_input
                        .exprs
                        .iter()
                        .flat_map(|expr| {
                            let arg_index = |var: &Attribute| {
                                if let Attribute::Arg(n) = var {
                                    Some(*n)
                                } else {
                                    None
                                }
                            };
                            expr.analyze(character, arg_index).active_args
                        })
                        .count()
                ),
            });
            continue;
        };

        let ctx = ArgsContext { character, args };
        for expression in &pending_input.exprs {
            if expression.eval_lenient(&ctx).is_err() {
                errors.push(ChoiceValidationError {
                    feature_name: pending_input.feature_name.to_string(),
                    message: format!("guard constraint failed with ARG values {args:?}"),
                });
                break;
            }
        }
    }
    errors
}

fn format_validation_feedback(errors: &[ChoiceValidationError]) -> String {
    let mut feedback = String::from("The following features have invalid ARG values:\n");
    for error in errors {
        let _ = writeln!(feedback, "- \"{}\": {}", error.feature_name, error.message);
    }
    feedback.push_str(
        "\nPlease respond with corrected JSON for ONLY these features. \
         Make sure all constraints are satisfied.",
    );
    feedback
}

#[component]
pub fn AiGenerateModal(
    show: RwSignal<bool>,
    on_result: Callback<AiGenerateResult>,
) -> impl IntoView {
    let description = RwSignal::new(String::new());
    let error_text = RwSignal::new(Option::<String>::None);
    let phase = RwSignal::new("");
    let show_settings = RwSignal::new(false);
    let settings = RwSignal::new(storage::load_ai_settings());

    let has_key = Memo::new(move |_| settings.get().has_api_key());

    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let generate = Action::new_local(move |description: &String| {
        let description = description.clone();
        let settings = settings.get_untracked();

        let species_list =
            registry.with_species_defs(|defs| join_iter(defs.keys().map(|name| &**name), ", "));

        let backgrounds_list =
            registry.with_background_defs(|defs| join_iter(defs.keys().map(|name| &**name), ", "));

        let classes_list = registry.with_class_defs(|defs| {
            join_iter(
                defs.values().map(|class_def| {
                    let subclasses =
                        join_iter(class_def.subclasses.values().map(|sub| &*sub.name), ", ");
                    if subclasses.is_empty() {
                        class_def.name.to_string()
                    } else {
                        format!("{} (subclasses: {subclasses})", class_def.name)
                    }
                }),
                "\n",
            )
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
                                <p class="ai-generate-no-key">{move_tr!("ai-generate-no-key")}</p>
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
                                    error_text
                                        .get()
                                        .map(|error| {
                                            view! {
                                                <p class="ai-generate-error">
                                                    {move_tr!("ai-generate-error")} ": " {error}
                                                </p>
                                            }
                                        })
                                }}

                                <div class="ai-generate-status">
                                    <Spinner loading=Signal::derive(move || generating.get()) />
                                    <Show when=move || generating.get()>
                                        <p class="ai-generate-phase">
                                            {move || {
                                                let i18n = expect_context::<I18n>();
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
                <button type="button" class="btn-link" on:click=move |_| show.set(false)>
                    {move_tr!("import-cancel")}
                </button>
                <button
                    type="button"
                    class="btn-icon"
                    title="AI Settings"
                    on:click=move |_| show_settings.set(true)
                >
                    <Icon name="settings" size=16 />
                </button>
            </div>
            <AiSettingsModal show=show_settings settings />
        </Modal>
    }
}

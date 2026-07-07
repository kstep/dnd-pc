use std::time::Duration;

use leptos::{leptos_dom::helpers::set_timeout, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    components::{
        ai_generate_modal::{AiGenerateModal, AiGenerateResult},
        apply::{apply_with_modal, apply_with_prefilled_args, mark_all_applied},
        icon::Icon,
        package_picker::PackagePicker,
        ref_link::Ref,
    },
    model::{
        Character, CharacterCore, CharacterStoreFields, FeatureCategory, FeatureSource,
        PersonalityStoreFields,
    },
    names::{self, NamesData},
    rules::{
        FeaturesView, RecomputePending, RulesRegistry,
        apply::{
            PICK_BACKGROUND, PICK_CLASS, PICK_SPECIES, PICK_SUBCLASS, PendingFeature,
            collect_pending_features,
        },
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
            let current = store.personality().name().get_untracked();
            if current == "New Character" {
                store.personality().name().set(data.generate_name());
            }
        }
    });

    let randomize_name = move |_| {
        if let Some(Some(ref data)) = *names_data.read_untracked() {
            store.personality().name().set(data.generate_name());
        }
    };

    // Label as `ArcSignal<String>` subscribes to the locale resource, so
    // switching language updates the rendered text without remounting.
    let generation_options: Memo<Vec<(String, ArcSignal<String>)>> = Memo::new(move |_| {
        registry.with_features_index(|idx| {
            idx.values()
                .filter(|feat| matches!(feat.category, FeatureCategory::Generation))
                .map(|feat| {
                    let (label, _) = registry.feature_label_desc(&feat.name);
                    (feat.name.to_string(), label)
                })
                .collect::<Vec<_>>()
        })
    });

    let generation_method = RwSignal::new(
        generation_options
            .read_untracked()
            .first()
            .map(|(name, _)| name.clone())
            .unwrap_or_default(),
    );

    Effect::new(move || {
        let first_name = generation_options.with(|options| {
            let valid = generation_method
                .with_untracked(|current| options.iter().any(|(name, _)| name == current));
            (!valid)
                .then(|| options.first().map(|(name, _)| name.clone()))
                .flatten()
        });
        if let Some(name) = first_name {
            generation_method.set(name);
        }
    });

    let on_create = move |_| {
        create_character(store, registry, generation_method);
    };

    let show_ai_modal = RwSignal::new(false);

    // AI result callback runs synchronously in Effect context — full Leptos
    // Owner available (navigate, ArgsModalCtx, etc.)
    let on_ai_result = Callback::new(move |result: AiGenerateResult| {
        apply_ai_result(store, registry, generation_method, result);
    });

    let skip_href = format!("/c/{}", store.read_untracked().id);

    view! {
        <AiGenerateModal show=show_ai_modal on_result=on_ai_result />
        <form
            class="quick-start-page"
            on:submit=move |event| {
                event.prevent_default();
                on_create(event);
            }
        >
            <div class="quick-start-header">
                <h2>{move_tr!("quick-start-title")}</h2>
                <button type="button" class="btn-primary" on:click=move |_| show_ai_modal.set(true)>
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
                        prop:value=move || store.personality().name().get()
                        on:input=move |event| {
                            store.personality().name().set(event_target_value(&event));
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
                <label>{move_tr!("rule-packages")}</label>
                <PackagePicker
                    value=Signal::derive(move || store.packages().get())
                    on_change=Callback::new(move |set| store.packages().set(set))
                    guard=store
                />
            </div>

            <div class="quick-start-section">
                <label>{move_tr!("quick-start-generation")}</label>
                <div class="generation-options">
                    {move || {
                        generation_options
                            .read()
                            .iter()
                            .map(|(name, label)| {
                                let name_for_check = name.clone();
                                let name_for_set = name.clone();
                                let label = label.clone();
                                view! {
                                    <label class="generation-option">
                                        <input
                                            type="radio"
                                            name="generation"
                                            value=name.clone()
                                            prop:checked=move || {
                                                generation_method.with(|method| method == &name_for_check)
                                            }
                                            on:change=move |_| {
                                                generation_method.set(name_for_set.clone())
                                            }
                                        />
                                        {move || label.get()}
                                    </label>
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </div>

            <div class="quick-start-actions">
                <button type="submit" class="btn-primary">
                    {move_tr!("quick-start-create")}
                </button>
                <Ref href=skip_href attr:class="btn-link">
                    {move_tr!("quick-start-skip")}
                </Ref>
            </div>
        </form>
    }
}

// --- Shared helpers ---

/// Initial pending list for the quick-start cascade modal. Order matches
/// the conventional creation flow: generation method → species →
/// background → class. The cascade-recompute closure re-runs this against
/// the speculative character so newly-relevant downstream features
/// (species traits, background skills, class L1 features) appear as soon
/// as the user makes a pick.
fn build_quick_start_pending_features(
    character: &CharacterCore,
    registry: &RulesRegistry,
    features_index: FeaturesView<'_>,
    gen_name: &str,
) -> Vec<PendingFeature> {
    let level = character.level().max(1);
    // Always include the four placeholder features unconditionally — they
    // own per-section reactive signals (replacement_choice, validity memos)
    // owned by their <For> child scope. Removing them from pending after
    // their identity slot fills would unmount the section, dispose the
    // signals, and leave dangling references in the watcher Effect's
    // all_replacements aggregator. The picker's replacement-choice memory
    // keeps the user's pick visible across cascade re-runs.
    let mut pending: Vec<PendingFeature> = Vec::new();
    if !gen_name.is_empty() {
        pending.push(PendingFeature {
            name: gen_name.into(),
            source: FeatureSource::User(0),
            level,
            replaces: None,
        });
    }
    for placeholder_name in [PICK_SPECIES, PICK_BACKGROUND] {
        pending.push(PendingFeature {
            name: placeholder_name.into(),
            source: FeatureSource::User(0),
            level: 0,
            replaces: None,
        });
    }
    // Class Level placeholder: source = User(target_total_level) so the
    // resulting System(Class) marker lands on User(1) for the first class,
    // matching what level_up_class emits for subsequent level-ups.
    pending.push(PendingFeature {
        name: PICK_CLASS.into(),
        source: FeatureSource::User(level),
        level,
        replaces: None,
    });
    pending.extend(collect_pending_features(
        character,
        registry,
        features_index,
    ));
    pending
}

fn collect_quick_start_pending(
    store: &Store<Character>,
    registry: &RulesRegistry,
    gen_name: &str,
) -> Vec<PendingFeature> {
    store.with_untracked(|character| {
        registry.with_features_index_untracked(|fi| {
            build_quick_start_pending_features(character, registry, fi, gen_name)
        })
    })
}

/// Post-apply hook for both manual and AI quick-start. The framework owns
/// the full apply cascade (outer pending + derived features); this hook
/// only flips the applied flags, sets starting HP, and navigates to the
/// editor.
fn finalize_quick_start(character: &mut Character) {
    mark_all_applied(character);
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

    let recompute = quick_start_recompute(registry, gen_name.clone());

    apply_with_modal(
        store,
        registry,
        all_pending,
        None,
        Some(recompute),
        finalize_quick_start,
    );
}

/// Recompute closure for the quick-start cascade. The modal's pick-watcher
/// invokes this against the speculative character (cascade base with
/// tentative species/background picks layered on top) so newly-relevant
/// features (Tiefling's L1 grants, Soldier's tool/skill picks, etc.) appear
/// in the modal as soon as the user makes the pick.
fn quick_start_recompute(registry: RulesRegistry, gen_name: String) -> RecomputePending {
    Box::new(move |speculative: &CharacterCore| {
        // Reset applied flags so collect/build_pending_features see fresh
        // class L_n features. Cascade-on-clone speculative path sets these
        // flags to feed identity events; we strip before discovery.
        let mut snapshot = speculative.clone();
        snapshot.applied.reset();
        registry.with_features_index_untracked(|fi| {
            build_quick_start_pending_features(&snapshot, &registry, fi, &gen_name)
                .into_iter()
                .filter_map(|pf| {
                    let feat_def = fi.get(&pf.name)?;
                    pf.pending_inputs(feat_def, &snapshot)
                })
                .collect()
        })
    })
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

    // Drive class / subclass acquisition through the placeholder
    // replacement-pick path. AI no longer pre-writes identity.classes
    // directly — the System(Class) and System(Subclass) features'
    // assigns set the values when the placeholders swap.
    let mut replacements = result.replacements;
    if !concept.class.is_empty() {
        replacements.insert(PICK_CLASS.into(), concept.class.clone());
    }
    if let Some(subclass) = concept.subclass.as_deref()
        && !subclass.is_empty()
    {
        replacements.insert(PICK_SUBCLASS.into(), subclass.to_string());
    }

    let recompute = quick_start_recompute(registry, preset_name);

    apply_with_prefilled_args(
        store,
        registry,
        all_pending,
        prefilled,
        replacements,
        Some(recompute),
        finalize_quick_start,
    );
}

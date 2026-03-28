use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;
use reactive_stores::Store;

use crate::{
    components::{
        background_field::BackgroundField, character_header::apply_with_args_modal,
        classes_section::ClassesSection, species_field::SpeciesField,
    },
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields, Feature},
    rules::RulesRegistry,
};

#[component]
pub fn QuickStart() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let generation_method = RwSignal::new(String::new());

    let generation_options = Memo::new(move |_| {
        let character = store.read();
        registry.with_features_index(|idx| {
            idx.values()
                .filter(|feat| {
                    feat.selectable
                        && feat.name.starts_with("Generation:")
                        && feat.meets_prerequisites(&character)
                })
                .map(|feat| (feat.name.clone(), feat.label().to_string()))
                .collect::<Vec<_>>()
        })
    });

    let generation_applied = Memo::new(move |_| {
        let features = store.features().read();
        features
            .iter()
            .any(|feature| feature.name.starts_with("Generation:") && feature.applied)
    });

    let apply_generation = move |_| {
        let name = generation_method.get_untracked();
        if name.is_empty() {
            return;
        }

        // Add the feature to the character's feature list
        store.features().write().push(Feature {
            name: name.clone(),
            ..Feature::default()
        });

        // Apply it (may open ArgsModal for Point Buy / Preset)
        if let Some(pending) =
            store.with_untracked(|character| registry.feature_needs_args(character, &name))
        {
            // Store name in a signal so the closure is Copy
            let feat_name = StoredValue::new(name);
            apply_with_args_modal(vec![pending], move |args_map| {
                let name = feat_name.get_value();
                let identity = store.with_untracked(|character| character.identity.clone());
                let level = store.with_untracked(|character| character.level());
                registry.with_feature_source(&identity, &name, |feat_def, source| {
                    let args = args_map.and_then(|map| map.get(name.as_str()).cloned());
                    store.update(|character| {
                        feat_def.apply_with_args(level, character, source.as_ref(), args)
                    });
                });
            });
        } else {
            let identity = store.with_untracked(|character| character.identity.clone());
            let level = store.with_untracked(|character| character.level());
            registry.with_feature_source(&identity, &name, |feat_def, source| {
                store.update(|character| feat_def.apply(level, character, source.as_ref()));
            });
        }
    };

    let on_done = move |_| {
        let id = store.read_untracked().id;
        let navigate = use_navigate();
        navigate(&format!("/c/{id}"), Default::default());
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
                <Show when=move || {
                    !generation_method.get().is_empty() && !generation_applied.get()
                }>
                    <button class="btn-primary" on:click=apply_generation>
                        {move_tr!("btn-apply-feature")}
                    </button>
                </Show>
                <Show when=move || generation_applied.get()>
                    <span class="generation-applied">{move_tr!("quick-start-applied")}</span>
                </Show>
            </div>

            <div class="quick-start-section">
                <label>{move_tr!("species")}</label>
                <SpeciesField />
            </div>

            <div class="quick-start-section">
                <label>{move_tr!("background")}</label>
                <BackgroundField />
            </div>

            <ClassesSection />

            <div class="quick-start-actions">
                <button class="btn-primary" on:click=on_done>
                    {move_tr!("quick-start-done")}
                </button>
            </div>
        </div>
    }
}

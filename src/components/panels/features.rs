use leptos::{either::Either, prelude::*};
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::{
        args_modal::ArgsModalCtx, datalist_input::DatalistInput, icon::Icon, panel::Panel,
        toggle_button::ToggleButton,
    },
    model::{
        Character, CharacterIdentityStoreFields, CharacterStoreFields, Feature, FeatureSource,
    },
    rules::{DefinitionStore, RulesRegistry},
};

#[component]
pub fn FeaturesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let features = store.features();

    let add_feature = move |_| {
        features.write().push(Feature::default());
    };

    let feature_options = Memo::new(move |_| {
        let character = store.read();
        registry.with_features_index(|features_index| {
            features_index
                .values()
                .filter(|feat| feat.selectable && feat.meets_prerequisites(&character))
                .map(|feat| {
                    (
                        feat.name.clone(),
                        feat.label().to_string(),
                        feat.description.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });

    view! {
        <Panel title=move_tr!("panel-features") class="features-panel">
            <div class="entry-list">
                {move || {
                    let classes_list = store.identity().classes().read();
                    let feature_data = store.feature_data().read();
                    let options = feature_options;
                    features
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, feature)| {
                            let name = feature.label().to_string();
                            let desc = feature.description.clone();
                            let is_readonly = feature_data
                                .get(&feature.name)
                                .and_then(|fd| fd.source.as_ref())
                                .is_some()
                                || registry.with_features_index(|idx| {
                                    idx.get(feature.name.as_str())
                                        .is_some_and(|f| !f.selectable)
                                });
                            let source_text = feature_data.get(&feature.name).and_then(|fd| {
                                fd.source.as_ref().map(|src| {
                                    let (prefix, label) = match src {
                                        FeatureSource::Class(class_name) => {
                                            let label = classes_list.iter()
                                                .find(|c| c.class == *class_name)
                                                .map(|c| c.class_label().to_string())
                                                .unwrap_or_else(|| class_name.clone());
                                            (tr!("source-class"), label)
                                        }
                                        FeatureSource::Species(species_name) => {
                                            let label = registry.species().with(species_name, |d| {
                                                d.label.as_deref().unwrap_or(&d.name).to_string()
                                            }).unwrap_or_else(|| species_name.clone());
                                            (tr!("source-species"), label)
                                        }
                                        FeatureSource::Background(bg_name) => {
                                            let label = registry.backgrounds().with(bg_name, |d| {
                                                d.label.as_deref().unwrap_or(&d.name).to_string()
                                            }).unwrap_or_else(|| bg_name.clone());
                                            (tr!("source-background"), label)
                                        }
                                    };
                                    format!("{prefix}: {label}")
                                })
                            });
                            view! {
                                <div class="entry-item">
                                    <ToggleButton />
                                    <div class="entry-content">
                                        {if is_readonly {
                                            Either::Left(view! {
                                                <span class="entry-name entry-name-readonly">{name.clone()}</span>
                                            })
                                        } else {
                                            Either::Right(view! {
                                                <DatalistInput
                                                    value=name
                                                    placeholder=move_tr!("feature-name")
                                                    class="entry-name"
                                                    options=options
                                                    on_input=move |input, resolved| {
                                                        let mut w = features.write();
                                                        if let Some(key) = resolved {
                                                            w[i].name = key;
                                                            w[i].label = None;
                                                        } else {
                                                            w[i].set_label(input);
                                                        }
                                                        w[i].description.clear();
                                                    }
                                                />
                                            })
                                        }}
                                    </div>
                                    <div class="entry-actions">
                                        <button
                                            class="btn-apply-level"
                                            title=move_tr!("btn-apply-feature")
                                            on:click=move |_| {
                                                let name = features.read()[i].name.clone();
                                                let (level, identity) = store.with_untracked(|c| {
                                                    let level = registry
                                                        .feature_class_level(&c.identity, &name)
                                                        .unwrap_or_else(|| c.level());
                                                    (level, c.identity.clone())
                                                });
                                                if let Some(pending) = store.with_untracked(|c| registry.feature_needs_args(c, &name)) {
                                                    let args_ctx = expect_context::<ArgsModalCtx>();
                                                    let name = name.clone();
                                                    args_ctx.open(vec![pending], move |args_map| {
                                                        registry.with_feature_source(&identity, &name, |feat_def, source| {
                                                            let args = args_map.get(name.as_str()).cloned();
                                                            store.update(|c| feat_def.apply_with_args(level, c, source.as_ref(), args));
                                                        });
                                                    });
                                                } else if registry.with_feature_source(&identity, &name, |feat_def, source| {
                                                    store.update(|c| feat_def.apply(level, c, source.as_ref()));
                                                }).is_none() {
                                                    log::warn!("Feature {name} not found in index, registry may not be loaded yet");
                                                }
                                            }
                                        >
                                            <Icon name="arrow-up" size=14 />
                                        </button>
                                        <button
                                            class="btn-remove"
                                            on:click=move |_| {
                                                if i < features.read().len() {
                                                    let removed = features.write().remove(i);
                                                    if !features.read().iter().any(|f| f.name == removed.name) {
                                                        store.feature_data().write().remove(&removed.name);
                                                    }
                                                }
                                            }
                                        >
                                            <Icon name="x" size=14 />
                                        </button>
                                    </div>
                                    {source_text.as_ref().map(|s| view! {
                                        <span class="entry-sublabel">{s.clone()}</span>
                                    })}
                                    <textarea
                                        class="entry-desc"
                                        placeholder=move_tr!("description")
                                        prop:value=desc.clone()
                                        on:change=move |e| {
                                            features.write()[i].description = event_target_value(&e);
                                        }
                                    />
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button class="btn-add" on:click=add_feature>
                {move_tr!("btn-add-feature")}
            </button>
        </Panel>
    }
}

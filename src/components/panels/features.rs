use leptos::{either::Either, prelude::*};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::{
        args_modal::ArgsModalCtx, datalist_input::DatalistInput, icon::Icon, panel::Panel,
        toggle_button::ToggleButton,
    },
    model::{Character, CharacterStoreFields, Feature, FeatureSource},
    rules::{RulesRegistry, WhenCondition},
};

#[component]
pub fn FeaturesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<I18n>();

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
                    let options = feature_options;
                    features
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, feature)| {
                            let name = feature.label().to_string();
                            let desc = feature.description.clone();
                            let source = &feature.source;
                            let is_readonly = !matches!(source, FeatureSource::User(_))
                                || registry.with_features_index(|idx| {
                                    idx.get(feature.name.as_str())
                                        .is_some_and(|f| !f.selectable)
                                });
                            let source_text = source.display_name(i18n);
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
                                                let level = store.with_untracked(|character| {
                                                    registry
                                                        .feature_class_level(&character.identity, &name)
                                                        .unwrap_or_else(|| character.level())
                                                });
                                                if let Some(pending) = store.with_untracked(|c| registry.feature_needs_args(c, &name)) {
                                                    let args_ctx = expect_context::<ArgsModalCtx>();
                                                    let name = name.clone();
                                                    args_ctx.open(vec![pending], move |inputs| {
                                                        registry.with_feature(&name, |feat_def| {
                                                            let feature_inputs = inputs.get(&name);
                                                            store.update(|c| {
                                                                c.mark_feature_applied(&name, feat_def.label.clone(), feat_def.description.clone(), FeatureSource::User(level));
                                                                feat_def.apply(level, c, WhenCondition::OnFeatureAdd, feature_inputs);
                                                            });
                                                        });
                                                    });
                                                } else if registry.with_feature(&name, |feat_def| {
                                                    store.update(|c| {
                                                        c.mark_feature_applied(&name, feat_def.label.clone(), feat_def.description.clone(), FeatureSource::User(level));
                                                        feat_def.apply(level, c, WhenCondition::OnFeatureAdd, &[]);
                                                    });
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
            <button class="btn-primary" on:click=add_feature>
                {move_tr!("btn-add-feature")}
            </button>
        </Panel>
    }
}

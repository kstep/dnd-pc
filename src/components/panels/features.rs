use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::{
        datalist_input::DatalistInput, icon::Icon, panel::Panel, toggle_button::ToggleButton,
    },
    model::{
        Character, CharacterIdentityStoreFields, CharacterStoreFields, Feature, FeatureSource,
        RacialTrait,
    },
    rules::{DefinitionStore, RulesRegistry},
};

#[component]
pub fn FeaturesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let features = store.features();
    let racial_traits = store.racial_traits();
    let expanded = RwSignal::new(HashSet::<usize>::new());
    let rt_expanded = RwSignal::new(HashSet::<usize>::new());

    let add_feature = move |_| {
        features.write().push(Feature::default());
    };

    let feature_options = Memo::new(move |_| {
        let classes = store.identity().classes().read();
        classes
            .iter()
            .filter_map(|cl| {
                registry.classes().with(&cl.class, |def| {
                    def.features(cl.subclass.as_deref())
                        .map(|feat| {
                            (
                                feat.name.clone(),
                                feat.label().to_string(),
                                feat.description.clone(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .flatten()
            .collect::<Vec<_>>()
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
                                        FeatureSource::Race(race_name) => {
                                            let label = registry.races().with(race_name, |d| {
                                                d.label.as_deref().unwrap_or(&d.name).to_string()
                                            }).unwrap_or_else(|| race_name.clone());
                                            (tr!("source-race"), label)
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
                            let is_open = Signal::derive(move || expanded.get().contains(&i));
                            view! {
                                <div class="entry-item">
                                    <ToggleButton
                                        expanded=is_open
                                        on_toggle=move || expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                    />
                                    <div class="entry-content">
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
                                                if registry.with_feature_source(&identity, &name, |feat_def, source| {
                                                    store.update(|c| feat_def.apply(level, c, &source));
                                                }).is_none() {
                                                    log::warn!("Cannot determine source for feature {name}, registry may not be loaded yet");
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
                                    <Show when=move || is_open.get()>
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
                                    </Show>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button class="btn-add" on:click=add_feature>
                {move_tr!("btn-add-feature")}
            </button>

            <h4>{move_tr!("racial-traits")}</h4>
            <div class="entry-list">
                {move || {
                    racial_traits
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, rt)| {
                            let name = rt.label().to_string();
                            let desc = rt.description.clone();
                            let is_open = Signal::derive(move || rt_expanded.get().contains(&i));
                            view! {
                                <div class="entry-item">
                                    <ToggleButton
                                        expanded=is_open
                                        on_toggle=move || rt_expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                    />
                                    <div class="entry-content">
                                        <input
                                            type="text"
                                            class="entry-name"
                                            placeholder=move_tr!("trait-name")
                                            prop:value=name
                                            on:change=move |e| {
                                                racial_traits.write()[i].set_label(event_target_value(&e));
                                            }
                                        />
                                    </div>
                                    <div class="entry-actions">
                                        <button
                                            class="btn-remove"
                                            on:click=move |_| {
                                                if i < racial_traits.read().len() {
                                                    racial_traits.write().remove(i);
                                                }
                                            }
                                        >
                                            <Icon name="x" size=14 />
                                        </button>
                                    </div>
                                    <Show when=move || is_open.get()>
                                        <textarea
                                            class="entry-desc"
                                            placeholder=move_tr!("description")
                                            prop:value=desc.clone()
                                            on:change=move |e| {
                                                racial_traits.write()[i].description = event_target_value(&e);
                                            }
                                        />
                                    </Show>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button
                class="btn-add"
                on:click=move |_| {
                    racial_traits.write().push(RacialTrait::default());
                }
            >
                {move_tr!("btn-add-racial-trait")}
            </button>
        </Panel>
    }
}

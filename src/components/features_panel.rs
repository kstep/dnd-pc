use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{panel::Panel, toggle_button::ToggleButton},
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields, Feature, RacialTrait},
    rules::RulesRegistry,
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

    view! {
        <Panel title=move_tr!("panel-features") class="features-panel">
            {move || {
                let classes = store.identity().classes().read();
                let options: Vec<(String, String)> = classes.iter().filter_map(|c| {
                    registry.with_class(&c.class, |def| {
                        def.features(c.subclass.as_deref())
                            .map(|f| (f.name.clone(), f.description.clone()))
                            .collect::<Vec<_>>()
                    })
                }).flatten().collect();
                view! {
                    <datalist id="feature-suggestions">
                        {options.into_iter().map(|(name, desc)| {
                            view! { <option value=name>{desc}</option> }
                        }).collect_view()}
                    </datalist>
                }
            }}

            <div class="features-list">
                {move || {
                    features
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, feature)| {
                            let name = feature.name.clone();
                            let desc = feature.description.clone();
                            let is_open = Signal::derive(move || expanded.get().contains(&i));
                            view! {
                                <div class="feature-entry">
                                    <ToggleButton
                                        expanded=is_open
                                        on_toggle=move || expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                    />
                                    <input
                                        type="text"
                                        class="feature-name"
                                        list="feature-suggestions"
                                        placeholder=move_tr!("feature-name")
                                        prop:value=name
                                        on:input=move |e| {
                                            let name = event_target_value(&e);
                                            let classes = store.identity().classes().read();
                                            let desc = classes.iter().find_map(|c| {
                                                registry.with_class(&c.class, |def| {
                                                    def.find_feature(&name, c.subclass.as_deref())
                                                        .map(|f| f.description.clone())
                                                })?
                                            });
                                            drop(classes);
                                            let mut w = features.write();
                                            w[i].name = name;
                                            if let Some(desc) = desc {
                                                w[i].description = desc;
                                            }
                                        }
                                    />
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
                                            registry.with_feature(&identity, &name, |feat_def| {
                                                store.update(|c| feat_def.apply(level, c));
                                            });
                                        }
                                    >
                                        "\u{2B06}"
                                    </button>
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            if i < features.read().len() {
                                                features.write().remove(i);
                                            }
                                        }
                                    >
                                        "X"
                                    </button>
                                    <Show when=move || is_open.get()>
                                        <textarea
                                            class="feature-desc"
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
            <div class="features-list">
                {move || {
                    racial_traits
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, rt)| {
                            let name = rt.name.clone();
                            let desc = rt.description.clone();
                            let is_open = Signal::derive(move || rt_expanded.get().contains(&i));
                            view! {
                                <div class="feature-entry">
                                    <ToggleButton
                                        expanded=is_open
                                        on_toggle=move || rt_expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                    />
                                    <input
                                        type="text"
                                        class="feature-name"
                                        placeholder=move_tr!("trait-name")
                                        prop:value=name
                                        on:input=move |e| {
                                            racial_traits.write()[i].name = event_target_value(&e);
                                        }
                                    />
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            if i < racial_traits.read().len() {
                                                racial_traits.write().remove(i);
                                            }
                                        }
                                    >
                                        "X"
                                    </button>
                                    <Show when=move || is_open.get()>
                                        <textarea
                                            class="feature-desc"
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

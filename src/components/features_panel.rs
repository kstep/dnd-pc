use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::panel::Panel,
    model::{Character, CharacterStoreFields, Feature},
};

#[component]
pub fn FeaturesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let features = store.features();
    let expanded = RwSignal::new(HashSet::<usize>::new());

    let add_feature = move |_| {
        features.write().push(Feature::default());
    };

    view! {
        <Panel title=move_tr!("panel-features") class="features-panel">
            <div class="features-list">
                {move || {
                    features
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, feature)| {
                            let name = feature.name.clone();
                            let desc = feature.description.clone();
                            let is_open = move || expanded.get().contains(&i);
                            let toggle = move |_| {
                                expanded.update(|set| {
                                    if !set.remove(&i) {
                                        set.insert(i);
                                    }
                                });
                            };
                            view! {
                                <div class="feature-entry">
                                    <input
                                        type="text"
                                        class="feature-name"
                                        placeholder=move_tr!("feature-name")
                                        prop:value=name
                                        on:input=move |e| {
                                            features.write()[i].name = event_target_value(&e);
                                        }
                                    />
                                    <button
                                        class="btn-toggle-desc"
                                        on:click=toggle
                                    >
                                        {move || if is_open() { "\u{2212}" } else { "+" }}
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
                                    <Show when=is_open>
                                        <textarea
                                            class="feature-desc"
                                            placeholder=move_tr!("description")
                                            prop:value=desc.clone()
                                            on:input=move |e| {
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
        </Panel>
    }
}

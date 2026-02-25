use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::panel::Panel,
    model::{Character, CharacterStoreFields, Feature},
};

#[component]
pub fn FeaturesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let features = store.features();

    let add_feature = move |_| {
        features.write().push(Feature::default());
    };

    view! {
        <Panel title="Features & Traits" class="features-panel">
            <div class="features-list">
                {move || {
                    features
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, feature)| {
                            let name = feature.name.clone();
                            let desc = feature.description.clone();
                            view! {
                                <div class="feature-entry">
                                    <input
                                        type="text"
                                        class="feature-name"
                                        placeholder="Feature name"
                                        prop:value=name
                                        on:input=move |e| {
                                            features.write()[i].name = event_target_value(&e);
                                        }
                                    />
                                    <textarea
                                        class="feature-desc"
                                        placeholder="Description"
                                        prop:value=desc
                                        on:input=move |e| {
                                            features.write()[i].description = event_target_value(&e);
                                        }
                                    />
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
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button class="btn-add" on:click=add_feature>
                "+ Add Feature"
            </button>
        </Panel>
    }
}

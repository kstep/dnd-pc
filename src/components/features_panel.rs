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
                            let show_desc = RwSignal::new(false);
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
                                    <button
                                        class="btn-toggle-desc"
                                        on:click=move |_| show_desc.update(|v| *v = !*v)
                                    >
                                        {move || if show_desc.get() { "\u{2212}" } else { "+" }}
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
                                    <Show when=move || show_desc.get()>
                                        <textarea
                                            class="feature-desc"
                                            placeholder="Description"
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
                "+ Add Feature"
            </button>
        </Panel>
    }
}

use leptos::prelude::*;

use crate::model::{Character, Feature};

#[component]
pub fn FeaturesPanel() -> impl IntoView {
    let char_signal = expect_context::<RwSignal<Character>>();

    let features = Memo::new(move |_| char_signal.get().features.clone());

    let add_feature = move |_| {
        char_signal.update(|c| {
            c.features.push(Feature::default());
        });
    };

    view! {
        <div class="panel features-panel">
            <h3>"Features & Traits"</h3>
            <div class="features-list">
                {move || {
                    features
                        .get()
                        .into_iter()
                        .enumerate()
                        .map(|(i, feature)| {
                            view! {
                                <div class="feature-entry">
                                    <input
                                        type="text"
                                        class="feature-name"
                                        placeholder="Feature name"
                                        prop:value=feature.name.clone()
                                        on:input=move |e| {
                                            char_signal.update(|c| {
                                                if let Some(f) = c.features.get_mut(i) {
                                                    f.name = event_target_value(&e);
                                                }
                                            });
                                        }
                                    />
                                    <textarea
                                        class="feature-desc"
                                        placeholder="Description"
                                        prop:value=feature.description.clone()
                                        on:input=move |e| {
                                            char_signal.update(|c| {
                                                if let Some(f) = c.features.get_mut(i) {
                                                    f.description = event_target_value(&e);
                                                }
                                            });
                                        }
                                    />
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            char_signal.update(|c| {
                                                if i < c.features.len() {
                                                    c.features.remove(i);
                                                }
                                            });
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
        </div>
    }
}

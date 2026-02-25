use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{panel::Panel, toggle_button::ToggleButton},
    model::{Character, CharacterStoreFields, Feature, RacialTrait},
};

#[component]
pub fn FeaturesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let features = store.features();
    let racial_traits = store.racial_traits();
    let expanded = RwSignal::new(HashSet::<usize>::new());
    let rt_expanded = RwSignal::new(HashSet::<usize>::new());

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
                                        placeholder=move_tr!("feature-name")
                                        prop:value=name
                                        on:input=move |e| {
                                            features.write()[i].name = event_target_value(&e);
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

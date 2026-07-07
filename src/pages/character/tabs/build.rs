use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{package_picker::PackagePicker, panels::features::FeaturesPanel},
    model::{Character, CharacterStoreFields},
};

#[component]
pub fn BuildTab() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    view! {
        <div class="editor-tab">
            <div class="panel package-picker-panel">
                <label>{move_tr!("rule-packages")}</label>
                <PackagePicker
                    value=Signal::derive(move || store.packages().get())
                    on_change=Callback::new(move |set| store.packages().set(set))
                    guard=store
                />
            </div>
            <FeaturesPanel />
        </div>
    }
}

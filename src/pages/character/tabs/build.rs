use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::{package_picker::PackagePickerPanel, panels::features::FeaturesPanel},
    model::{Character, CharacterStoreFields},
};

#[component]
pub fn BuildTab() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    view! {
        <div class="editor-tab">
            <PackagePickerPanel
                value=Signal::derive(move || store.packages().get())
                on_change=Callback::new(move |set| store.packages().set(set))
                guard=store
            />
            <FeaturesPanel />
        </div>
    }
}

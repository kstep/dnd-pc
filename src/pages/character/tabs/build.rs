use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::{package_picker::CharacterPackagePanel, panels::features::FeaturesPanel},
    model::Character,
};

#[component]
pub fn BuildTab() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    view! {
        <div class="editor-tab">
            <CharacterPackagePanel store wrapper_class="panel package-picker-panel" />
            <FeaturesPanel />
        </div>
    }
}

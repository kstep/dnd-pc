use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::panel::Panel,
    model::{Character, CharacterStoreFields},
};

#[component]
pub fn NotesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    view! {
        <Panel title="Notes" class="notes-panel">
            <textarea
                class="notes-textarea"
                prop:value=move || store.notes().get()
                on:input=move |e| {
                    store.notes().set(event_target_value(&e));
                }
            />
        </Panel>
    }
}

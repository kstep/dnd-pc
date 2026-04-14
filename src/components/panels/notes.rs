use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::model::{Character, CharacterStoreFields};

#[component]
pub fn NotesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    view! {
        <section>
            <h3>{move_tr!("panel-notes")}</h3>
            <textarea
                class="notes-textarea"
                prop:value=move || store.notes().get()
                on:input=move |e| {
                    store.notes().set(event_target_value(&e));
                }
            />
        </section>
    }
}

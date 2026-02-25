use leptos::prelude::*;

use crate::{components::panel::Panel, model::Character};

#[component]
pub fn NotesPanel() -> impl IntoView {
    let char_signal = expect_context::<RwSignal<Character>>();

    let notes = Memo::new(move |_| char_signal.get().notes.clone());

    view! {
        <Panel title="Notes" class="notes-panel">
            <textarea
                class="notes-textarea"
                prop:value=notes
                on:input=move |e| {
                    char_signal.update(|c| c.notes = event_target_value(&e));
                }
            />
        </Panel>
    }
}

use leptos::prelude::*;

use crate::model::Character;

#[component]
pub fn NotesPanel() -> impl IntoView {
    let char_signal = expect_context::<RwSignal<Character>>();

    let notes = Memo::new(move |_| char_signal.get().notes.clone());

    view! {
        <div class="panel notes-panel">
            <h3>"Notes"</h3>
            <textarea
                class="notes-textarea"
                prop:value=notes
                on:input=move |e| {
                    char_signal.update(|c| c.notes = event_target_value(&e));
                }
            />
        </div>
    }
}

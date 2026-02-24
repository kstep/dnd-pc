use leptos::prelude::*;

use crate::model::Character;

#[component]
pub fn ProficienciesPanel() -> impl IntoView {
    let char_signal = expect_context::<RwSignal<Character>>();

    let text = Memo::new(move |_| char_signal.get().proficiencies_and_languages.clone());

    view! {
        <div class="panel proficiencies-panel">
            <h3>"Proficiencies & Languages"</h3>
            <textarea
                class="proficiencies-textarea"
                prop:value=text
                on:input=move |e| {
                    char_signal.update(|c| c.proficiencies_and_languages = event_target_value(&e));
                }
            />
        </div>
    }
}

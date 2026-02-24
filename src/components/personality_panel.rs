use leptos::prelude::*;

use crate::model::Character;

#[component]
pub fn PersonalityPanel() -> impl IntoView {
    let char_signal = expect_context::<RwSignal<Character>>();

    let traits = Memo::new(move |_| char_signal.get().personality.personality_traits.clone());
    let ideals = Memo::new(move |_| char_signal.get().personality.ideals.clone());
    let bonds = Memo::new(move |_| char_signal.get().personality.bonds.clone());
    let flaws = Memo::new(move |_| char_signal.get().personality.flaws.clone());

    view! {
        <div class="panel personality-panel">
            <h3>"Personality"</h3>
            <div class="textarea-field">
                <label>"Personality Traits"</label>
                <textarea
                    prop:value=traits
                    on:input=move |e| {
                        char_signal.update(|c| c.personality.personality_traits = event_target_value(&e));
                    }
                />
            </div>
            <div class="textarea-field">
                <label>"Ideals"</label>
                <textarea
                    prop:value=ideals
                    on:input=move |e| {
                        char_signal.update(|c| c.personality.ideals = event_target_value(&e));
                    }
                />
            </div>
            <div class="textarea-field">
                <label>"Bonds"</label>
                <textarea
                    prop:value=bonds
                    on:input=move |e| {
                        char_signal.update(|c| c.personality.bonds = event_target_value(&e));
                    }
                />
            </div>
            <div class="textarea-field">
                <label>"Flaws"</label>
                <textarea
                    prop:value=flaws
                    on:input=move |e| {
                        char_signal.update(|c| c.personality.flaws = event_target_value(&e));
                    }
                />
            </div>
        </div>
    }
}

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::panel::Panel,
    model::{Character, CharacterStoreFields, PersonalityStoreFields},
};

#[component]
pub fn PersonalityPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let personality = store.personality();

    view! {
        <Panel title="Personality" class="personality-panel">
            <div class="textarea-field">
                <label>"Personality Traits"</label>
                <textarea
                    prop:value=move || personality.personality_traits().get()
                    on:input=move |e| {
                        personality.personality_traits().set(event_target_value(&e));
                    }
                />
            </div>
            <div class="textarea-field">
                <label>"Ideals"</label>
                <textarea
                    prop:value=move || personality.ideals().get()
                    on:input=move |e| {
                        personality.ideals().set(event_target_value(&e));
                    }
                />
            </div>
            <div class="textarea-field">
                <label>"Bonds"</label>
                <textarea
                    prop:value=move || personality.bonds().get()
                    on:input=move |e| {
                        personality.bonds().set(event_target_value(&e));
                    }
                />
            </div>
            <div class="textarea-field">
                <label>"Flaws"</label>
                <textarea
                    prop:value=move || personality.flaws().get()
                    on:input=move |e| {
                        personality.flaws().set(event_target_value(&e));
                    }
                />
            </div>
        </Panel>
    }
}

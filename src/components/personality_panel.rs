use leptos::prelude::*;
use leptos_fluent::move_tr;
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
        <Panel title=move_tr!("panel-personality") class="personality-panel">
            <div class="textarea-field">
                <label>{move_tr!("history")}</label>
                <textarea
                    prop:value=move || personality.history().get()
                    on:input=move |e| {
                        personality.history().set(event_target_value(&e));
                    }
                />
            </div>
            <div class="textarea-field">
                <label>{move_tr!("personality-traits")}</label>
                <textarea
                    prop:value=move || personality.personality_traits().get()
                    on:input=move |e| {
                        personality.personality_traits().set(event_target_value(&e));
                    }
                />
            </div>
            <div class="textarea-field">
                <label>{move_tr!("ideals")}</label>
                <textarea
                    prop:value=move || personality.ideals().get()
                    on:input=move |e| {
                        personality.ideals().set(event_target_value(&e));
                    }
                />
            </div>
            <div class="textarea-field">
                <label>{move_tr!("bonds")}</label>
                <textarea
                    prop:value=move || personality.bonds().get()
                    on:input=move |e| {
                        personality.bonds().set(event_target_value(&e));
                    }
                />
            </div>
            <div class="textarea-field">
                <label>{move_tr!("flaws")}</label>
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

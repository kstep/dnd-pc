use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    ai::{self, AiSettings, AiSettingsStoreFields},
    components::modal::Modal,
    storage,
};

#[component]
pub fn AiSettingsModal(show: RwSignal<bool>, settings: RwSignal<AiSettings>) -> impl IntoView {
    let draft = Store::new(settings.get_untracked());

    let fetch_trigger = RwSignal::new(0u32);

    let remote_models = LocalResource::new(move || {
        let version = fetch_trigger.get();
        let current = settings.get_untracked();
        async move {
            if version == 0 || !current.has_api_key() {
                return Vec::new();
            }
            ai::fetch_models(&current).await.unwrap_or_default()
        }
    });

    Effect::new(move || {
        if show.get() {
            draft.set(settings.get_untracked());
            fetch_trigger.update(|v| *v += 1);
        }
    });

    let on_save = move |_| {
        let saved = draft.get();
        storage::save_ai_settings(&saved);
        settings.set(saved);
        show.set(false);
    };

    let models_list = move || -> Vec<String> {
        let remote = remote_models.read();
        match remote.as_deref() {
            Some(models) if !models.is_empty() => models.to_vec(),
            _ => vec![draft.model().get()],
        }
    };

    view! {
        <Modal show title=move_tr!("story-settings")>
            <div class="modal-body ai-settings-modal">
                <div class="textarea-field">
                    <label>
                        {move_tr!("story-api-key")}
                        " "
                        <a href="https://platform.openai.com/api-keys" target="_blank">
                            {move_tr!("story-get-key")}
                        </a>
                    </label>
                    <input
                        type="text"
                        autocomplete="off"
                        class="secret-input"
                        prop:value=move || draft.api_key().get()
                        on:input=move |event| {
                            draft.api_key().set(event_target_value(&event));
                        }
                    />
                </div>
                <div class="textarea-field">
                    <label>{move_tr!("story-model")}</label>
                    <Suspense fallback=move || view! {
                        <select disabled>
                            <option>{move || draft.model().get()} " ⏳"</option>
                        </select>
                    }>
                        {move || {
                            let current_model = draft.model().get();
                            let models = models_list();
                            view! {
                                <select on:change=move |event| {
                                    draft.model().set(event_target_value(&event));
                                }>
                                    {models.into_iter().map(|model| {
                                        let selected = model == *current_model;
                                        let label = model.clone();
                                        view! { <option value=model selected=selected>{label}</option> }
                                    }).collect::<Vec<_>>()}
                                </select>
                            }
                        }}
                    </Suspense>
                </div>
            </div>
            <div class="modal-actions">
                <button class="btn-primary" on:click=on_save>{move_tr!("story-save")}</button>
            </div>
        </Modal>
    }
}

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    ai::{self, AiError, AiProvider, AiSettings, AiSettingsStoreFields, ModelLists},
    components::modal::Modal,
    firebase, storage,
};

#[component]
pub fn AiSettingsModal(show: RwSignal<bool>, settings: RwSignal<AiSettings>) -> impl IntoView {
    let draft = Store::new(settings.get_untracked());
    let proxy_available = AiProvider::Proxy.is_available();

    let remote_models = LocalResource::new(move || {
        let api_key = draft.api_key().get();
        let provider = draft.provider().get();
        async move {
            let can_probe = match provider {
                AiProvider::OpenAI => !api_key.trim().is_empty(),
                AiProvider::Proxy => firebase::current_provider().as_deref() == Some("google.com"),
            };
            if !can_probe {
                return Ok(ModelLists::default());
            }
            ai::fetch_models(&AiSettings {
                provider,
                api_key,
                model: String::new(),
                image_model: String::new(),
            })
            .await
        }
    });

    Effect::new(move || {
        if show.get() {
            draft.set(settings.get_untracked());
        }
    });

    let on_save = move |_| {
        let saved = draft.get();
        storage::save_ai_settings(&saved);
        settings.set(saved);
        show.set(false);
    };

    let fetch_error = move || -> Option<String> {
        remote_models
            .read()
            .as_ref()
            .and_then(|result: &Result<ModelLists, AiError>| {
                result.as_ref().err().map(|error| error.to_string())
            })
    };

    let chat_models_list = move || -> Vec<String> {
        let remote = remote_models.read();
        match remote.as_ref().and_then(|result| result.as_ref().ok()) {
            Some(lists) if !lists.chat.is_empty() => lists.chat.clone(),
            _ => vec![draft.model().get()],
        }
    };

    let image_models_list = move || -> Vec<String> {
        let remote = remote_models.read();
        match remote.as_ref().and_then(|result| result.as_ref().ok()) {
            Some(lists) if !lists.image.is_empty() => lists.image.clone(),
            _ => vec![draft.image_model().get()],
        }
    };

    let is_proxy = move || matches!(draft.provider().get(), AiProvider::Proxy);

    view! {
        <Modal show title=move_tr!("story-settings")>
            <div class="modal-body ai-settings-modal">
                <Show when=move || proxy_available>
                    <div class="textarea-field">
                        <label>
                            <input
                                type="checkbox"
                                prop:checked=is_proxy
                                on:change=move |event| {
                                    draft.provider().set(if event_target_checked(&event) {
                                        AiProvider::Proxy
                                    } else {
                                        AiProvider::OpenAI
                                    });
                                }
                            />
                            " " {move_tr!("ai-settings-provider-hosted")}
                        </label>
                    </div>
                </Show>
                <Show when=is_proxy fallback=move || view! {
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
                            aria-invalid=move || fetch_error().is_some().then_some("true")
                            prop:value=move || draft.api_key().get()
                            on:change=move |event| draft.api_key().set(event_target_value(&event))
                        />
                    </div>
                }>
                    <Show when=move || firebase::current_provider().as_deref() != Some("google.com")>
                        <div class="textarea-field">
                            <p class="field-hint">{move_tr!("ai-settings-google-required")}</p>
                            <button
                                type="button"
                                class="btn-primary"
                                on:click=move |_| storage::sign_in_with_google()
                            >
                                {move_tr!("hint-sign-in-button")}
                            </button>
                        </div>
                    </Show>
                </Show>
                {move || fetch_error().map(|err| view! {
                    <p class="field-error">
                        {move_tr!("ai-settings-fetch-failed")}
                        ": "
                        {err}
                    </p>
                })}
                <div class="textarea-field">
                    <label>{move_tr!("story-model")}</label>
                    <Suspense fallback=move || view! {
                        <select disabled>
                            <option>{move || draft.model().get()} " ⏳"</option>
                        </select>
                    }>
                        {move || {
                            let current_model = draft.model().get();
                            let models = chat_models_list();
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
                <div class="textarea-field">
                    <label>{move_tr!("ai-settings-image-model")}</label>
                    <Suspense fallback=move || view! {
                        <select disabled>
                            <option>{move || draft.image_model().get()} " ⏳"</option>
                        </select>
                    }>
                        {move || {
                            let current_model = draft.image_model().get();
                            let models = image_models_list();
                            view! {
                                <select on:change=move |event| {
                                    draft.image_model().set(event_target_value(&event));
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

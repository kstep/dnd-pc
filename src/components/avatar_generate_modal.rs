use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    ai::{self, AiSettings, CharacterContext},
    components::{ai_settings_modal::AiSettingsModal, icon::Icon, modal::Modal, spinner::Spinner},
    model::{Avatar as AvatarData, Character, now_epoch_secs},
    storage::{self, image::process_image_base64},
};

#[component]
pub fn AvatarGenerateModal(
    show: RwSignal<bool>,
    char_id: Uuid,
    on_result: Callback<AvatarData>,
) -> impl IntoView {
    let extra = RwSignal::new(String::new());
    let error_text = RwSignal::new(Option::<String>::None);
    let show_settings = RwSignal::new(false);
    let settings = RwSignal::new(storage::load_ai_settings());

    let has_key = Memo::new(move |_| settings.with(AiSettings::has_api_key));

    let store = expect_context::<Store<Character>>();

    let generate = Action::new_local(move |extra: &String| {
        let extra = extra.clone();
        let current_settings = settings.get_untracked();
        let size = ai::image_size_for(&current_settings.image_model);
        let prompt = store.with_untracked(|character| {
            ai::build_avatar_prompt(&CharacterContext::from_character(character), &extra, size)
        });

        async move {
            let b64 = ai::generate_avatar_image(&current_settings, &prompt).await?;
            let data_uri = process_image_base64(&b64).await?;
            Ok::<AvatarData, ai::AiError>(AvatarData {
                id: char_id,
                data_uri: data_uri.into(),
                updated_at: now_epoch_secs(),
            })
        }
    });

    Effect::new(move || {
        let value = generate.value().read();
        let Some(ref result) = *value else { return };
        match result {
            Ok(avatar) => {
                on_result.run(avatar.clone());
                extra.set(String::new());
                show.set(false);
            }
            Err(error) => {
                error_text.set(Some(error.to_string()));
            }
        }
    });

    let generating = generate.pending();

    Effect::new(move || {
        if show.get() {
            error_text.set(None);
        }
    });

    let on_generate = move |_| {
        if has_key.get_untracked() && !generating.get_untracked() {
            generate.dispatch(extra.get_untracked());
        }
    };

    view! {
        <Modal show title=move_tr!("avatar-generate-title")>
            <div class="modal-body avatar-generate-modal">
                {move || {
                    if !has_key.get() {
                        Either::Left(
                            view! {
                                <p class="ai-generate-no-key">{move_tr!("ai-generate-no-key")}</p>
                            },
                        )
                    } else {
                        Either::Right(
                            view! {
                                <div class="textarea-field">
                                    <label>{move_tr!("avatar-generate-description")}</label>
                                    <textarea
                                        rows="3"
                                        placeholder=move_tr!("avatar-generate-placeholder")
                                        prop:value=move || extra.get()
                                        on:input=move |event| {
                                            extra.set(event_target_value(&event));
                                        }
                                        prop:disabled=move || generating.get()
                                    />
                                </div>

                                {move || {
                                    error_text
                                        .get()
                                        .map(|error| {
                                            view! {
                                                <p class="ai-generate-error">
                                                    {move_tr!("avatar-generate-failed")} ": " {error}
                                                </p>
                                            }
                                        })
                                }}

                                <div class="ai-generate-status">
                                    <Spinner loading=Signal::derive(move || generating.get()) />
                                    <Show when=move || generating.get()>
                                        <p class="ai-generate-phase">
                                            {move_tr!("avatar-generate-phase-rendering")}
                                        </p>
                                    </Show>
                                </div>
                            },
                        )
                    }
                }}
            </div>
            <div class="modal-actions">
                <Show when=move || has_key.get()>
                    <button
                        type="button"
                        class="btn-primary"
                        prop:disabled=move || generating.get()
                        on:click=on_generate
                    >
                        <Icon name="sparkles" size=16 />
                        " "
                        {move_tr!("avatar-generate-button")}
                    </button>
                </Show>
                <button type="button" class="btn-link" on:click=move |_| show.set(false)>
                    {move_tr!("import-cancel")}
                </button>
                <button
                    type="button"
                    class="btn-icon"
                    title="AI Settings"
                    on:click=move |_| show_settings.set(true)
                >
                    <Icon name="settings" size=16 />
                </button>
            </div>
            <AiSettingsModal show=show_settings settings />
        </Modal>
    }
}

use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::{
    components::A,
    hooks::{use_navigate, use_params},
    params::Params,
};
use reactive_stores::Store;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;

use crate::{
    BASE_URL,
    ai::{AiSettings, CharacterContext, Story, generate_story},
    components::{icon::Icon, modal::Modal},
    model::Character,
    pages::reference::ReferenceSidebar,
    storage,
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct StoryParams {
    story_id: Option<Uuid>,
}

// --- Settings Modal ---

#[component]
fn AiSettingsModal(show: RwSignal<bool>, settings: RwSignal<AiSettings>) -> impl IntoView {
    let draft = RwSignal::new(settings.get_untracked());

    Effect::new(move || {
        if show.get() {
            draft.set(settings.get_untracked());
        }
    });

    let on_save = move |_| {
        let saved = draft.get_untracked();
        storage::save_ai_settings(&saved);
        settings.set(saved);
        show.set(false);
    };

    view! {
        <Modal show title=move_tr!("story-settings")>
            <div class="ai-settings-modal">
                <div class="modal-body">
                    <div class="textarea-field">
                        <label>
                            {move_tr!("story-api-key")}
                            " "
                            <a href="https://platform.openai.com/api-keys" target="_blank">
                                {move_tr!("story-get-key")}
                            </a>
                        </label>
                        <input
                            type="password"
                            prop:value=move || draft.get().api_key
                            on:input=move |event| {
                                draft.update(|draft| draft.api_key = event_target_value(&event));
                            }
                        />
                    </div>
                    <div class="textarea-field">
                        <label>{move_tr!("story-model")}</label>
                        <input
                            type="text"
                            prop:value=move || draft.get().model
                            on:input=move |event| {
                                draft.update(|draft| draft.model = event_target_value(&event));
                            }
                        />
                    </div>
                </div>
                <div class="modal-actions">
                    <button class="btn-primary" on:click=on_save>{move_tr!("story-save")}</button>
                </div>
            </div>
        </Modal>
    }
}

// --- Story Sidebar ---

#[component]
fn StorySidebar(char_id: Uuid, stories: RwSignal<Vec<Story>>) -> impl IntoView {
    let current_label = Signal::derive(String::new);

    view! {
        <ReferenceSidebar current_label>
            <A
                href=format!("{BASE_URL}/c/{char_id}/story")
                exact=true
                attr:class="reference-nav-item story-nav-new"
            >
                {move_tr!("story-new")}
            </A>
            <For
                each=move || stories.get()
                key=|story| story.id
                let:story
            >
                <A
                    href=format!("{BASE_URL}/c/{char_id}/story/{}", story.id)
                    attr:class="reference-nav-item"
                >
                    <span class="story-nav-title">{story.title.clone()}</span>
                    <span class="story-nav-date">{story.short_date().to_string()}</span>
                </A>
            </For>
        </ReferenceSidebar>
    }
}

// --- New Story View ---

#[component]
fn NewStoryView(
    char_id: Uuid,
    stories: RwSignal<Vec<Story>>,
    settings: RwSignal<AiSettings>,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let show_settings = RwSignal::new(false);
    let prompt = RwSignal::new(String::new());
    let streaming_text = RwSignal::new(String::new());
    let is_streaming = RwSignal::new(false);
    let error_msg = RwSignal::new(Option::<String>::None);

    let has_key = move || settings.get().has_api_key();

    let build_context = move || {
        let character = store.get();
        CharacterContext {
            name: character.identity.name.clone(),
            species: character.identity.species.clone(),
            class_summary: character.class_summary(),
            level: character.level(),
            history: character.personality.history.clone(),
            personality_traits: character.personality.personality_traits.clone(),
            ideals: character.personality.ideals.clone(),
            bonds: character.personality.bonds.clone(),
            flaws: character.personality.flaws.clone(),
            notes: character.notes.clone(),
        }
    };

    let on_generate = move |_| {
        let ai_settings = settings.get_untracked();
        if !ai_settings.has_api_key() {
            return;
        }
        let user_prompt = prompt.get_untracked();
        if user_prompt.trim().is_empty() {
            return;
        }
        let context = build_context();

        is_streaming.set(true);
        error_msg.set(None);
        streaming_text.set(String::new());

        spawn_local(async move {
            let result = generate_story(&ai_settings, &context, &user_prompt, |chunk| {
                streaming_text.update(|text| text.push_str(chunk));
            })
            .await;

            is_streaming.set(false);

            match result {
                Ok(full_text) => {
                    let title = full_text
                        .lines()
                        .next()
                        .unwrap_or("Untitled")
                        .chars()
                        .take(60)
                        .collect::<String>();

                    let story = Story::new(title, user_prompt, full_text);
                    stories.update(|list| list.insert(0, story));
                    storage::save_stories(&char_id, &stories.get_untracked());
                    prompt.set(String::new());
                }
                Err(error) => {
                    error_msg.set(Some(error));
                }
            }
        });
    };

    view! {
        <div class="story-generate-view">
            <div class="story-output">
                {move || {
                    let text = streaming_text.get();
                    let err = error_msg.get();
                    if let Some(error) = err {
                        Either::Left(view! {
                            <div class="story-error">
                                <p><strong>{move_tr!("story-error")}</strong></p>
                                <p>{error}</p>
                            </div>
                        })
                    } else if text.is_empty() && !is_streaming.get() {
                        Either::Right(Either::Left(view! {
                            <p class="story-placeholder">{move_tr!("story-select")}</p>
                        }))
                    } else {
                        Either::Right(Either::Right(view! {
                            <div class="story-content">
                                <pre>{text}</pre>
                            </div>
                        }))
                    }
                }}
            </div>

            <div class="story-input">
                {move || {
                    if !has_key() {
                        Either::Left(view! {
                            <div class="story-no-key">
                                <p>{move_tr!("story-no-api-key")}</p>
                                <button on:click=move |_| show_settings.set(true)>
                                    {move_tr!("story-settings")}
                                </button>
                            </div>
                        })
                    } else {
                        Either::Right(view! {
                            <div class="story-prompt">
                                <textarea
                                    class="notes-textarea"
                                    placeholder=move_tr!("story-prompt-placeholder")
                                    prop:value=move || prompt.get()
                                    on:input=move |event| {
                                        prompt.set(event_target_value(&event));
                                    }
                                    disabled=move || is_streaming.get()
                                />
                                <div class="story-actions">
                                    <button
                                        on:click=on_generate
                                        disabled=move || is_streaming.get()
                                    >
                                        {move || if is_streaming.get() {
                                            move_tr!("story-stop")
                                        } else if error_msg.get().is_some() {
                                            move_tr!("story-retry")
                                        } else {
                                            move_tr!("story-generate")
                                        }}
                                    </button>
                                    <button
                                        class="btn-icon"
                                        title=move_tr!("story-settings")
                                        on:click=move |_| show_settings.set(true)
                                    >
                                        <Icon name="settings" size=18 />
                                    </button>
                                </div>
                            </div>
                        })
                    }
                }}
            </div>
        </div>
        <AiSettingsModal show=show_settings settings />
    }
}

// --- View Story ---

#[component]
fn ViewStoryView(char_id: Uuid, story_id: Uuid, stories: RwSignal<Vec<Story>>) -> impl IntoView {
    let story = Memo::new(move |_| stories.get().into_iter().find(|story| story.id == story_id));

    let navigate = use_navigate();

    view! {
        {move || story.get().map(|story| {
            let navigate = navigate.clone();
            let content = story.content.clone();

            let on_delete = move |_| {
                stories.update(|list| list.retain(|story| story.id != story_id));
                storage::save_stories(&char_id, &stories.get_untracked());
                navigate(&format!("{BASE_URL}/c/{char_id}/story"), Default::default());
            };

            let on_copy = move |_| {
                if let Some(window) = web_sys::window() {
                    let _ = window.navigator().clipboard().write_text(&content);
                }
            };

            view! {
                <div class="story-view">
                    <div class="story-view-header">
                        <h2>{story.title.clone()}</h2>
                        <span class="story-view-date">{story.short_date().to_string()}</span>
                    </div>
                    <div class="story-view-prompt">
                        <em>{story.prompt.clone()}</em>
                    </div>
                    <div class="story-content">
                        <pre>{story.content.clone()}</pre>
                    </div>
                    <div class="story-actions">
                        <button on:click=on_copy>
                            <Icon name="copy" size=16 />
                            {move_tr!("story-copy")}
                        </button>
                        <button class="btn-danger" on:click=on_delete>
                            <Icon name="trash-2" size=16 />
                            {move_tr!("story-delete")}
                        </button>
                    </div>
                </div>
            }
        })}
    }
}

// --- Main Story Page ---

#[component]
pub fn CharacterStory() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let char_id = store.read_untracked().id;
    let stories = RwSignal::new(storage::load_stories(&char_id));
    let settings = RwSignal::new(storage::load_ai_settings());
    let params = use_params::<StoryParams>();

    let story_id = move || params.get().ok().and_then(|params| params.story_id);

    view! {
        <div class="reference-page">
            <div class="reference-layout">
                <StorySidebar char_id stories />
                <main class="reference-main">
                    {move || match story_id() {
                        Some(sid) => Either::Left(view! {
                            <ViewStoryView char_id story_id=sid stories />
                        }),
                        None => Either::Right(view! {
                            <NewStoryView char_id stories settings />
                        }),
                    }}
                </main>
            </div>
        </div>
    }
}

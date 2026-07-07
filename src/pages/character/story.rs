use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::{
    hooks::{use_navigate, use_params},
    params::Params,
};
use reactive_stores::Store;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;

use crate::{
    ai::{AiSettings, CharacterContext, Story, generate_story},
    components::{ai_settings_modal::AiSettingsModal, icon::Icon, ref_link::Ref},
    model::Character,
    pages::reference::ReferenceSidebar,
    storage,
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct StoryParams {
    story_id: Option<Uuid>,
}

// --- Story Sidebar ---

#[component]
fn StorySidebar(char_id: Uuid, stories: RwSignal<Vec<Story>>) -> impl IntoView {
    let current_label = Signal::derive(String::new);

    view! {
        <ReferenceSidebar current_label>
            <Ref
                href=format!("/c/{char_id}/story")
                exact=true
                attr:class="reference-nav-item story-nav-new"
            >
                {move_tr!("story-new")}
            </Ref>
            <For each=move || stories.get() key=|story| story.id let:story>
                <Ref
                    href=format!("/c/{char_id}/story/{}", story.id)
                    attr:class="reference-nav-item"
                >
                    <span class="story-nav-title">{story.title.clone()}</span>
                    <span class="story-nav-date">{story.short_date().to_string()}</span>
                </Ref>
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

    let build_context = move || CharacterContext::from_character(&store.read());

    let cancelled = RwSignal::new(false);

    let on_generate = move |event: web_sys::SubmitEvent| {
        event.prevent_default();
        if is_streaming.get_untracked() {
            cancelled.set(true);
            return;
        }
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
        cancelled.set(false);
        error_msg.set(None);
        streaming_text.set(String::new());

        spawn_local(async move {
            let result = generate_story(&ai_settings, &context, &user_prompt, |chunk| {
                if !cancelled.get_untracked() {
                    streaming_text.update(|text| text.push_str(chunk));
                }
            })
            .await;

            is_streaming.set(false);

            if let Err(error) = result
                && !cancelled.get_untracked()
            {
                error_msg.set(Some(error.to_string()));
            }

            // Save whatever was generated (even partial if cancelled)
            let full_text = streaming_text.get_untracked();
            if !full_text.is_empty() {
                let first_line = full_text.lines().next().unwrap_or("Untitled");
                let title = if first_line.len() <= 80 {
                    first_line.to_string()
                } else {
                    let boundary = first_line.floor_char_boundary(80);
                    match first_line[..boundary].rfind(' ') {
                        Some(pos) => format!("{}…", &first_line[..pos]),
                        None => format!("{}…", &first_line[..boundary]),
                    }
                };

                let story = Story::new(title, user_prompt, full_text);
                stories.update(|list| list.insert(0, story));
                storage::save_stories(&char_id, &stories.get_untracked());
                if let Some(uid) = crate::firebase::current_uid() {
                    storage::queue::push(storage::queue::CloudOp::PushStories { uid, char_id });
                }
                prompt.set(String::new());
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
                        Either::Left(
                            view! {
                                <div class="story-error">
                                    <p>
                                        <strong>{move_tr!("story-error")}</strong>
                                    </p>
                                    <p>{error}</p>
                                </div>
                            },
                        )
                    } else if text.is_empty() && !is_streaming.get() {
                        Either::Right(
                            Either::Left(
                                view! {
                                    <p class="story-placeholder">{move_tr!("story-select")}</p>
                                },
                            ),
                        )
                    } else {
                        Either::Right(
                            Either::Right(
                                view! {
                                    <div class="story-content">
                                        <pre>{text}</pre>
                                    </div>
                                },
                            ),
                        )
                    }
                }}
            </div>

            <div class="story-input">
                {move || {
                    if !has_key() {
                        Either::Left(
                            view! {
                                <div class="story-no-key">
                                    <p>{move_tr!("story-no-api-key")}</p>
                                    <button on:click=move |_| {
                                        show_settings.set(true)
                                    }>{move_tr!("story-settings")}</button>
                                </div>
                            },
                        )
                    } else {
                        Either::Right(
                            view! {
                                <form class="story-prompt" on:submit=on_generate>
                                    <textarea
                                        class="notes-textarea"
                                        required
                                        placeholder=move_tr!("story-prompt-placeholder")
                                        prop:value=move || prompt.get()
                                        on:input=move |event| {
                                            prompt.set(event_target_value(&event));
                                        }
                                        disabled=move || is_streaming.get()
                                    />
                                    <div class="story-actions">
                                        <button type="submit" class="btn-primary">
                                            {move || {
                                                if is_streaming.get() {
                                                    move_tr!("story-stop")
                                                } else if error_msg.get().is_some() {
                                                    move_tr!("story-retry")
                                                } else {
                                                    move_tr!("story-generate")
                                                }
                                            }}
                                        </button>
                                        <button
                                            type="button"
                                            class="btn-icon"
                                            title=move_tr!("story-settings")
                                            on:click=move |_| show_settings.set(true)
                                        >
                                            <Icon name="settings" size=18 />
                                        </button>
                                    </div>
                                </form>
                            },
                        )
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
    let story = Memo::new(move |_| {
        stories.with(|list| list.iter().find(|story| story.id == story_id).cloned())
    });

    let navigate = use_navigate();

    view! {
        {move || {
            story
                .get()
                .map(|story| {
                    let navigate = navigate.clone();
                    let on_delete = move |_| {
                        stories.update(|list| list.retain(|story| story.id != story_id));
                        storage::save_stories(&char_id, &stories.get_untracked());
                        if let Some(uid) = crate::firebase::current_uid() {
                            storage::queue::push(storage::queue::CloudOp::DeleteStory {
                                uid,
                                char_id,
                                story_id,
                            });
                        }
                        navigate(&format!("/c/{char_id}/story"), Default::default());
                    };
                    let on_copy = move |_| {
                        let content = stories
                            .with(|list| {
                                list.iter()
                                    .find(|story| story.id == story_id)
                                    .map(|story| story.content.clone())
                            });
                        if let Some(text) = content {
                            crate::export::copy_to_clipboard(&text);
                        }
                    };

                    view! {
                        <div class="story-view">
                            <div class="story-view-header">
                                <h2>{story.title.clone()}</h2>
                                <div class="story-view-date">{story.short_date().to_string()}</div>
                            </div>
                            <div class="story-view-prompt">
                                <em>{story.prompt.clone()}</em>
                            </div>
                            <div class="story-content">
                                <pre>{story.content}</pre>
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
                })
        }}
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
                        Some(sid) => {
                            Either::Left(view! { <ViewStoryView char_id story_id=sid stories /> })
                        }
                        None => Either::Right(view! { <NewStoryView char_id stories settings /> }),
                    }}
                </main>
            </div>
        </div>
    }
}

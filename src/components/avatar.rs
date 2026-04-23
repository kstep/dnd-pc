use leptos::{ev::KeyboardEvent, html::Div, prelude::*};
use leptos_fluent::tr;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;

use crate::{
    components::{icon::Icon, toast::Toast},
    model::{Avatar as AvatarData, now_epoch_secs},
    storage::{
        image::{monogram_hue, monogram_initials, process_image_file},
        pick_file,
    },
};

#[component]
pub fn Avatar(
    #[prop(into)] name: Signal<String>,
    #[prop(into)] avatar: Signal<Option<AvatarData>>,
    /// Character UUID — embedded in the produced `Avatar` so sync can identify
    /// the doc without a separate lookup.
    char_id: Uuid,
    #[prop(default = 80)] size: u32,
    #[prop(default = false)] editable: bool,
    #[prop(into, optional)] on_change: Option<Callback<AvatarData>>,
    #[prop(into, optional)] on_remove: Option<Callback<()>>,
    #[prop(into, optional)] on_generate: Option<Callback<()>>,
) -> impl IntoView {
    let initials = Memo::new(move |_| name.with(|n| monogram_initials(n)));
    let hue = Memo::new(move |_| name.with(|n| monogram_hue(n)));
    let style = move || format!("--avatar-w: {size}px");

    let expanded = RwSignal::new(false);
    let avatar_ref = NodeRef::<Div>::new();

    let open_picker = move |event: web_sys::MouseEvent| {
        event.stop_propagation();
        pick_file("image/*", move |file| {
            spawn_local(async move {
                match process_image_file(file).await {
                    Ok(data_uri) => {
                        if let Some(callback) = on_change {
                            let avatar = AvatarData {
                                id: char_id,
                                data_uri: data_uri.into(),
                                updated_at: now_epoch_secs(),
                            };
                            callback.run(avatar);
                        }
                    }
                    Err(error) => {
                        log::error!("avatar processing failed: {error:?}");
                        Toast::i18n_error("avatar-load-failed").show();
                    }
                }
            });
        });
    };

    let on_generate_click = move |event: web_sys::MouseEvent| {
        event.stop_propagation();
        // Collapse the expand-modal before handing off to the AI generate
        // modal; stacking two modals is visually messy.
        expanded.set(false);
        if let Some(callback) = on_generate {
            callback.run(());
        }
    };

    let on_remove_click = move |event: web_sys::MouseEvent| {
        event.stop_propagation();
        if let Some(callback) = on_remove {
            callback.run(());
        }
    };

    let on_root_click = move |_| {
        if editable && !expanded.get_untracked() {
            expanded.set(true);
            // Move focus onto the div so the on:keydown ESC handler receives
            // events regardless of which element held focus before the click.
            if let Some(element) = avatar_ref.get() {
                let _ = element.focus();
            }
        }
    };

    let on_close_click = move |event: web_sys::MouseEvent| {
        event.stop_propagation();
        expanded.set(false);
    };

    let on_backdrop_click = move |event: web_sys::MouseEvent| {
        event.stop_propagation();
        expanded.set(false);
    };

    // ESC key dismissal while expanded.
    let on_keydown = move |event: KeyboardEvent| {
        if expanded.get_untracked() && event.key() == "Escape" {
            expanded.set(false);
        }
    };

    let has_avatar =
        Memo::new(move |_| avatar.with(|av| av.as_ref().is_some_and(|a| !a.is_empty())));

    view! {
        <div class="avatar"
             node_ref=avatar_ref
             class:avatar-editable=move || editable
             class:avatar-edit=move || expanded.get()
             style=style
             on:click=on_root_click
             on:keydown=on_keydown
             tabindex=move || if expanded.get() { "0" } else { "-1" }
        >
            <Show
                when=move || has_avatar.get()
                fallback=move || view! {
                    <Show
                        when=move || !initials.get().is_empty()
                        fallback=|| view! { <SilhouetteSvg /> }
                    >
                        <Monogram initials=initials hue=hue />
                    </Show>
                }
            >
                <img class="avatar-image"
                     src=move || avatar.with(|av| av.as_ref().map(|a| a.data_uri.clone()).unwrap_or_default())
                     alt="" />
            </Show>
            <Show when=move || editable>
                <button class="avatar-close"
                        on:click=on_close_click
                        aria-label=move || tr!("avatar-close")
                        title=move || tr!("avatar-close")
                        inert=move || !expanded.get()>
                    <Icon name="x" />
                </button>
                <div class="avatar-actions"
                     inert=move || !expanded.get()>
                    <button on:click=open_picker
                            aria-label=move || tr!("avatar-change")
                            title=move || tr!("avatar-change")>
                        <Icon name="camera" />
                    </button>
                    <Show when=move || on_generate.is_some()>
                        <button class="accent"
                                on:click=on_generate_click
                                aria-label=move || tr!("avatar-generate")
                                title=move || tr!("avatar-generate")>
                            <Icon name="sparkles" />
                        </button>
                    </Show>
                    <Show when=move || has_avatar.get()>
                        <button class="danger"
                                on:click=on_remove_click
                                aria-label=move || tr!("avatar-remove")
                                title=move || tr!("avatar-remove")>
                            <Icon name="trash-2" />
                        </button>
                    </Show>
                </div>
            </Show>
        </div>
        <Show when=move || expanded.get()>
            <div class="avatar-backdrop" on:click=on_backdrop_click></div>
        </Show>
    }
}

#[component]
fn Monogram(initials: Memo<String>, hue: Memo<u32>) -> impl IntoView {
    let bg = move || {
        let value = hue.get();
        format!(
            "background:linear-gradient(180deg, hsl({value}, 50%, 30%), hsl({value}, 50%, 18%))"
        )
    };
    view! {
        <div class="avatar-monogram" style=bg>
            {move || initials.get()}
        </div>
    }
}

#[component]
fn SilhouetteSvg() -> impl IntoView {
    view! {
        <svg class="avatar-silhouette" viewBox="0 0 24 24" fill="currentColor">
            <circle cx="12" cy="8" r="4"/>
            <path d="M3 22 C 3 16, 7 14, 9 14 L 12 19 L 15 14 C 17 14, 21 16, 21 22 Z"/>
        </svg>
    }
}

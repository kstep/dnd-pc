use leptos::{leptos_dom::helpers::set_timeout, prelude::*};
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
) -> impl IntoView {
    let initials = Memo::new(move |_| monogram_initials(&name.get()));
    let hue = Memo::new(move |_| monogram_hue(&name.get()));
    let style = move || format!("--avatar-w: {size}px");

    let touched = RwSignal::new(false);

    let open_picker = move |_| {
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
                        Toast::new(tr!("avatar-load-failed")).show();
                    }
                }
            });
        });
    };

    let on_remove_click = move |event: web_sys::MouseEvent| {
        event.stop_propagation();
        if let Some(callback) = on_remove {
            callback.run(());
        }
    };

    let on_touch = move |event: web_sys::TouchEvent| {
        // First tap on touch device: show overlay, suppress the synthetic
        // click that would otherwise fire the action immediately. Second tap
        // (while overlay is visible) lets click through to the action.
        if !touched.get_untracked() {
            event.prevent_default();
            touched.set(true);
            set_timeout(
                move || touched.set(false),
                std::time::Duration::from_millis(2500),
            );
        }
    };

    view! {
        <div class="avatar"
             class:avatar-editable=move || editable
             class:touched=move || touched.get()
             style=style
             on:touchstart=on_touch
        >
            <Show
                when=move || avatar.with(|av| av.as_ref().is_some_and(|a| !a.is_empty()))
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
                <button class="avatar-overlay"
                        on:click=open_picker
                        aria-label=move || tr!("avatar-change")>
                    <Icon name="camera" />
                </button>
                <Show when=move || avatar.with(|av| av.as_ref().is_some_and(|a| !a.is_empty()))>
                    <button class="avatar-remove"
                            on:click=on_remove_click
                            aria-label=move || tr!("avatar-remove")>
                        <Icon name="trash-2" />
                    </button>
                </Show>
            </Show>
        </div>
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

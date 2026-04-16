use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use uuid::Uuid;

use crate::{
    BASE_URL,
    components::avatar::Avatar as AvatarView,
    model::CharacterSummary,
    storage::{load_avatar, load_avatar_timestamp, sync_index_version},
};

#[component]
pub fn CharacterCard(
    summary: CharacterSummary,
    on_delete: impl Fn(Uuid) + Copy + 'static,
) -> impl IntoView {
    let id = summary.id;
    let avatar = RwSignal::new(load_avatar(&id));
    let index_version = sync_index_version();
    Effect::new(move |previous: Option<u32>| {
        let version = index_version.get();
        if previous.is_some() {
            let current_ts =
                avatar.with_untracked(|av| av.as_ref().map(|a| a.updated_at).unwrap_or(0));
            let new_ts = load_avatar_timestamp(&id).unwrap_or(0);
            if new_ts != current_ts {
                avatar.set(load_avatar(&id));
            }
        }
        version
    });
    let name = summary.name.clone();
    let href = format!("{BASE_URL}/c/{id}");
    let class_empty = summary.class.is_empty();
    let class_str = summary.class.clone();
    let flipped = RwSignal::new(false);
    let deleting = RwSignal::new(false);

    view! {
        <div class="character-card" class:card-flipped=flipped class:card-remove=deleting
             on:animationend=move |_| if deleting.get() { on_delete(id) }
        >
            <div class="card-inner">
                <div class="card-front">
                    <a href=href class="card-link">
                        <div class="card-row">
                            <AvatarView
                                name=name.clone()
                                avatar=avatar
                                char_id=id
                                size=64
                            />
                            <div class="card-text">
                                <h3>{name.clone()}</h3>
                                <p class="card-subtitle">
                                    {move_tr!("level-prefix")} " " {summary.level} " "
                                    <span>{move || if class_empty {
                                        tr!("no-class")
                                    } else {
                                        class_str.clone()
                                    }}</span>
                                </p>
                            </div>
                        </div>
                    </a>
                    <button
                        class="btn-danger"
                        on:click=move |event| {
                            event.prevent_default();
                            event.stop_propagation();
                            flipped.set(true);
                        }
                    >
                        {move_tr!("btn-delete")}
                    </button>
                </div>
                <div class="card-back">
                    <div>
                        <h3>{name}</h3>
                        <p class="card-subtitle">{move_tr!("confirm-delete")}</p>
                    </div>
                    <div class="card-back-buttons">
                        <button
                            on:click=move |event| {
                                event.prevent_default();
                                event.stop_propagation();
                                flipped.set(false);
                            }
                        >
                            {move_tr!("btn-cancel")}
                        </button>
                        <button
                            class="btn-danger"
                            on:click=move |event| {
                                event.prevent_default();
                                event.stop_propagation();
                                deleting.set(true);
                            }
                        >
                            {move_tr!("btn-confirm")}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}

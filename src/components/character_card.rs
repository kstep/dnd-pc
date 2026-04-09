use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use uuid::Uuid;

use crate::{BASE_URL, model::CharacterSummary};

#[component]
pub fn CharacterCard(
    summary: CharacterSummary,
    on_delete: impl Fn(Uuid) + Copy + 'static,
) -> impl IntoView {
    let id = summary.id;
    let href = format!("{BASE_URL}/c/{id}");
    let class_empty = summary.class.is_empty();
    let class_str = summary.class.clone();
    let name = summary.name.clone();
    let flipped = RwSignal::new(false);
    let deleting = RwSignal::new(false);

    view! {
        <div class="character-card" class:card-flipped=flipped class:card-remove=deleting
             on:animationend=move |_| if deleting.get() { on_delete(id) }
        >
            <div class="card-inner">
                <div class="card-front">
                    <a href=href class="card-link">
                        <h3>{name.clone()}</h3>
                        <p class="card-subtitle">
                            {move_tr!("level-prefix")} " " {summary.level} " "
                            <span>{move || if class_empty {
                                tr!("no-class")
                            } else {
                                class_str.clone()
                            }}</span>
                        </p>
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

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
    let deleting = RwSignal::new(false);

    view! {
        <div class="character-card" class:card-remove=deleting
             on:animationend=move |_| if deleting.get() { on_delete(id) }
        >
            <a href=href class="card-link">
                <h3>{summary.name.clone()}</h3>
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
                on:click=move |e| {
                    e.prevent_default();
                    e.stop_propagation();
                    deleting.set(true);
                }
            >
                {move_tr!("btn-delete")}
            </button>
        </div>
    }
}

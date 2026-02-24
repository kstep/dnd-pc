use leptos::prelude::*;
use leptos_router::components::A;
use uuid::Uuid;

use crate::model::CharacterSummary;

#[component]
pub fn CharacterCard(
    summary: CharacterSummary,
    on_delete: impl Fn(Uuid) + Copy + 'static,
) -> impl IntoView {
    let id = summary.id;
    let href = format!("/character/{id}");
    let display_class = if summary.class.is_empty() {
        "No class".to_string()
    } else {
        summary.class.clone()
    };

    view! {
        <div class="character-card">
            <A href=href attr:class="card-link">
                <h3>{summary.name.clone()}</h3>
                <p class="card-subtitle">
                    "Level " {summary.level} " " {display_class}
                </p>
            </A>
            <button
                class="btn-delete"
                on:click=move |e| {
                    e.prevent_default();
                    e.stop_propagation();
                    on_delete(id);
                }
            >
                "Delete"
            </button>
        </div>
    }
}

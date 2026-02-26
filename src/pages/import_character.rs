use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::{components::A, hooks::use_params_map};
use crate::{share, storage};

#[component]
pub fn ImportCharacter() -> impl IntoView {
    let params = use_params_map();
    let data = params.read().get("data");

    match data {
        Some(data) => match share::decode_character(&data) {
            Some(character) => {
                storage::save_character(&character);
                let id = character.id;

                let navigate = leptos_router::hooks::use_navigate();
                request_animation_frame(move || {
                    navigate(&format!("{}/c/{id}", crate::BASE_URL), Default::default());
                });

                view! { <p>"Importing..."</p> }.into_any()
            }
            None => view! {
                <div class="panel">
                    <h2>{move_tr!("share-error")}</h2>
                    <A href=format!("{}/", crate::BASE_URL)>{move_tr!("back-to-list")}</A>
                </div>
            }
            .into_any(),
        },
        None => view! {
            <div class="panel">
                <h2>{move_tr!("share-error")}</h2>
                <A href=format!("{}/", crate::BASE_URL)>{move_tr!("back-to-list")}</A>
            </div>
        }
        .into_any(),
    }
}

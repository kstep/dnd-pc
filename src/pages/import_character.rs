use leptos::prelude::*;
use leptos_fluent::{move_tr, tr};
use leptos_router::{
    components::A,
    hooks::{use_navigate, use_params_map},
};

use crate::{BASE_URL, model::Character, share, storage};

fn do_import(character: &Character) -> impl IntoView {
    storage::save_character(character);
    let id = character.id;

    let navigate = use_navigate();
    request_animation_frame(move || {
        navigate(&format!("{BASE_URL}/c/{id}"), Default::default());
    });

    view! { <p>"Importing..."</p> }
}

#[component]
fn ImportConflict(character: Character, existing_name: String) -> impl IntoView {
    let id = character.id;
    let character = StoredValue::new(character);

    let import_anyway = move |_| {
        storage::save_character(&character.get_value());
        let navigate = use_navigate();
        navigate(&format!("{BASE_URL}/c/{id}"), Default::default());
    };

    let message = tr!("import-conflict-message", { "name" => existing_name });

    view! {
        <div class="panel">
            <h2>{move_tr!("import-conflict-title")}</h2>
            <p>{message}</p>
            <div class="import-conflict-actions">
                <button on:click=import_anyway>{move_tr!("import-anyway")}</button>
                <A href=format!("{BASE_URL}/")>{move_tr!("import-cancel")}</A>
            </div>
        </div>
    }
}

#[component]
pub fn ImportCharacter() -> impl IntoView {
    let params = use_params_map();
    let data = params.read().get("data");

    match data {
        Some(data) => match share::decode_character(&data) {
            Some(character) => {
                let existing = storage::load_character(&character.id);
                let has_conflict = existing
                    .as_ref()
                    .is_some_and(|e| e.updated_at > character.updated_at);

                if has_conflict {
                    let existing_name = existing.unwrap().identity.name;
                    view! {
                        <ImportConflict character=character existing_name=existing_name />
                    }
                    .into_any()
                } else {
                    do_import(&character).into_any()
                }
            }
            None => view! {
                <div class="panel">
                    <h2>{move_tr!("share-error")}</h2>
                    <A href=format!("{BASE_URL}/")>{move_tr!("back-to-list")}</A>
                </div>
            }
            .into_any(),
        },
        None => view! {
            <div class="panel">
                <h2>{move_tr!("share-error")}</h2>
                <A href=format!("{BASE_URL}/")>{move_tr!("back-to-list")}</A>
            </div>
        }
        .into_any(),
    }
}

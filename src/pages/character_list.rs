use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;
use wasm_bindgen::prelude::*;

use crate::{
    components::character_card::CharacterCard,
    model::Character,
    pages::import_character::{ImportConflict, do_import},
    storage,
};

#[component]
pub fn CharacterList() -> impl IntoView {
    let (characters, set_characters) = signal(storage::load_index().characters);
    let import_state = RwSignal::new(None::<Character>);

    let create_character = move |_| {
        let mut character = Character::new();
        storage::save_character(&mut character);
        let id = character.id;
        set_characters.set(storage::load_index().characters);
        let navigate = use_navigate();
        navigate(&format!("/c/{id}"), Default::default());
    };

    let delete_character = move |id: uuid::Uuid| {
        storage::delete_character(&id);
        set_characters.set(storage::load_index().characters);
    };

    let load_from_file = move |_| {
        let document = leptos::prelude::document();
        let input: web_sys::HtmlInputElement =
            document.create_element("input").unwrap().unchecked_into();

        input.set_type("file");
        input.set_accept(".json");

        let input_clone = input.clone();
        let closure = Closure::<dyn Fn()>::new(move || {
            let Some(files) = input_clone.files() else {
                return;
            };
            let Some(file) = files.get(0) else {
                return;
            };

            let reader = match web_sys::FileReader::new() {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Failed to create FileReader: {e:?}");
                    return;
                }
            };

            let reader_clone = reader.clone();
            let onload = Closure::<dyn Fn()>::new(move || {
                let result = match reader_clone.result() {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("Failed to read file: {e:?}");
                        return;
                    }
                };
                let Some(text) = result.as_string() else {
                    log::error!("File result is not a string");
                    return;
                };
                match serde_json::from_str::<Character>(&text) {
                    Ok(character) => {
                        import_state.set(Some(character));
                    }
                    Err(e) => {
                        log::error!("Failed to parse character JSON: {e}");
                        leptos::prelude::window()
                            .alert_with_message(&format!("Invalid character file: {e}"))
                            .ok();
                    }
                }
            });

            reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            onload.forget();

            if let Err(e) = reader.read_as_text(&file) {
                log::error!("Failed to start reading file: {e:?}");
            }
        });

        input.set_onchange(Some(closure.as_ref().unchecked_ref()));
        closure.forget();

        input.click();
    };

    view! {
        {move || {
            match import_state.get() {
                Some(character) => {
                    let existing = storage::load_character(&character.id);
                    let has_conflict = existing
                        .as_ref()
                        .is_some_and(|ex| ex.updated_at > character.updated_at);
                    if has_conflict {
                        let existing = existing.unwrap();
                        view! { <ImportConflict incoming=character existing=existing /> }.into_any()
                    } else {
                        do_import(&character).into_any()
                    }
                }
                None => view! {
                    <div class="character-list-page">
                        <div class="character-list-header">
                            <h1>{move_tr!("page-characters")}</h1>
                            <button class="btn-create" on:click=create_character>
                                {move_tr!("btn-new-character")}
                            </button>
                            <button class="btn-add" on:click=load_from_file>
                                {move_tr!("btn-load-character")}
                            </button>
                        </div>
                        <div class="character-list">
                            <For
                                each=move || characters.get()
                                key=|c| c.id
                                let:character
                            >
                                <CharacterCard summary=character on_delete=delete_character />
                            </For>
                        </div>
                    </div>
                }
                .into_any(),
            }
        }}
    }
}

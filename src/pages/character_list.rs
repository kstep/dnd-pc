use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::hooks::use_navigate;

use crate::{components::character_card::CharacterCard, model::Character, storage};

#[component]
pub fn CharacterList() -> impl IntoView {
    let (characters, set_characters) = signal(storage::load_index().characters);

    let create_character = move |_| {
        let character = Character::new();
        storage::save_character(&character);
        let id = character.id;
        set_characters.set(storage::load_index().characters);
        let navigate = use_navigate();
        navigate(&format!("/character/{id}"), Default::default());
    };

    let delete_character = move |id: uuid::Uuid| {
        storage::delete_character(&id);
        set_characters.set(storage::load_index().characters);
    };

    view! {
        <div class="character-list-page">
            <h1>{move_tr!("page-characters")}</h1>
            <button class="btn-create" on:click=create_character>
                {move_tr!("btn-new-character")}
            </button>
            <div class="character-list">
                <For
                    each=move || characters.get()
                    key=|c| c.id
                    let:character
                >
                    <CharacterCard
                        summary=character
                        on_delete=delete_character
                    />
                </For>
            </div>
        </div>
    }
}

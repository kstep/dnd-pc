use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::hooks::use_navigate;

use crate::{
    components::{character_card::CharacterCard, navbar::ViewClass},
    model::Character,
    pages::import_character::import_or_conflict,
    storage,
};

#[component]
pub fn CharacterList() -> impl IntoView {
    expect_context::<ViewClass>().0.set("view-main".into());
    let i18n = expect_context::<leptos_fluent::I18n>();
    let (characters, set_characters) = signal(storage::load_index());
    let import_state = RwSignal::new(None::<Character>);

    // Re-read index when cloud pull updates it.
    let index_version = storage::sync_index_version();
    Effect::new(move |prev: Option<u32>| {
        if prev.is_some() {
            set_characters.set(storage::load_index());
        }
        index_version.get()
    });

    let create_character = move |_| {
        let mut character = Character::new();
        storage::save_and_sync_character(&mut character);
        let id = character.id;
        set_characters.set(storage::load_index());
        let navigate = use_navigate();
        navigate(&format!("/c/{id}"), Default::default());
    };

    let delete_character = move |id: uuid::Uuid| {
        storage::delete_character(&id);
        set_characters.set(storage::load_index());
    };

    let load_from_file = move |_| {
        storage::pick_character_from_file(move |character| import_state.set(Some(character)));
    };

    view! {
        <Title text=Signal::derive(move || i18n.tr("page-characters")) />
        {move || {
            if let Some(character) = import_state.get() {
                return Either::Left(import_or_conflict(character));
            }
            Either::Right(view! {
                    <div class="character-list-page">
                        <div class="character-list-actions">
                            <button class="btn-primary" on:click=create_character>
                                {move_tr!("btn-new-character")}
                            </button>
                            <button class="btn-primary" on:click=load_from_file>
                                {move_tr!("btn-load-character")}
                            </button>
                        </div>
                        <div class="character-list">
                            <For
                                each=move || characters.get().characters.sorted_unstable_by(|_, ch1, _, ch2| ch1.name.cmp(&ch2.name)).map(|(_, ch)| ch)
                                key=|c| c.id
                                let:character
                            >
                                <CharacterCard summary=character on_delete=delete_character />
                            </For>
                        </div>
                    </div>
                })
        }}
    }
}

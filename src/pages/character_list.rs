use leptos::{either::EitherOf3, prelude::*};
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_navigate};

use crate::{
    BASE_URL,
    components::character_card::CharacterCard,
    model::Character,
    pages::import_character::{ImportConflict, do_import},
    storage,
};

#[component]
pub fn CharacterList() -> impl IntoView {
    let i18n = expect_context::<leptos_fluent::I18n>();
    let (characters, set_characters) = signal(storage::load_index().characters);
    let import_state = RwSignal::new(None::<Character>);

    // Re-read index when cloud pull updates it.
    let index_version = storage::sync_index_version();
    Effect::new(move |prev: Option<u32>| {
        if prev.is_some() {
            set_characters.set(storage::load_index().characters);
        }
        index_version.get()
    });

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
        storage::pick_character_from_file(move |character| import_state.set(Some(character)));
    };

    view! {
        <Title text=Signal::derive(move || i18n.tr("page-characters")) />
        {move || {
            if let Some(character) = import_state.get() {
                let existing = storage::load_character(&character.id);
                let has_conflict = existing
                    .as_ref()
                    .is_some_and(|ex| ex.updated_at > character.updated_at);
                return if has_conflict {
                    let existing = existing.unwrap();
                    EitherOf3::A(view! { <ImportConflict incoming=character existing=existing /> })
                } else {
                    EitherOf3::B(do_import(character))
                };
            }
            EitherOf3::C(view! {
                    <div class="character-list-page">
                        <div class="character-list-header">
                            <h1>{move_tr!("page-characters")}</h1>
                        </div>
                        <div class="character-list-actions">
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
                        <div class="reference-links">
                            <h2>{move_tr!("ref-reference")}</h2>
                            <nav class="reference-links-nav">
                                <A href=format!("{BASE_URL}/r/class")>{move_tr!("ref-classes")}</A>
                                <A href=format!("{BASE_URL}/r/race")>{move_tr!("ref-races")}</A>
                                <A href=format!("{BASE_URL}/r/background")>{move_tr!("ref-backgrounds")}</A>
                                <A href=format!("{BASE_URL}/r/spell")>{move_tr!("ref-spells")}</A>
                            </nav>
                        </div>
                    </div>
                })
        }}
    }
}

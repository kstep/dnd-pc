use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::hooks::use_navigate;

use crate::{
    components::{character_card::CharacterCard, cloud_sign_in_hint::CloudSignInHint},
    model::Character,
    pages::import_character::import_or_conflict,
    rules::ActivePackages,
    storage,
};

#[component]
pub fn CharacterList() -> impl IntoView {
    let i18n = expect_context::<leptos_fluent::I18n>();
    let load_summaries = || {
        let mut summaries = storage::load_all_summaries();
        summaries.sort_by(|a, b| a.name.cmp(&b.name));
        summaries
    };
    let (characters, set_characters) = signal(load_summaries());
    let import_state = RwSignal::new(None::<Character>);

    // Re-read when cloud pull updates characters.
    let index_version = storage::sync_index_version();
    Effect::new(move |prev: Option<u32>| {
        if prev.is_some() {
            set_characters.set(load_summaries());
        }
        index_version.get()
    });

    let active_packages = expect_context::<ActivePackages>();
    let create_character = move |_| {
        let mut character = Character::new();
        character.packages = active_packages.0.get_untracked();
        storage::save_and_sync_character(&mut character);
        let id = character.id;
        set_characters.set(load_summaries());
        let navigate = use_navigate();
        navigate(&format!("/c/{id}/quick-start"), Default::default());
    };

    let delete_character = move |id: uuid::Uuid| {
        storage::delete_character(&id);
        set_characters.set(load_summaries());
    };

    let load_from_file = move |_| {
        storage::pick_character_from_file(move |character| import_state.set(Some(character)));
    };

    view! {
        <Title text=move_tr!(i18n, "page-characters") />
        {move || {
            if let Some(character) = import_state.get() {
                return Either::Left(import_or_conflict(character, None));
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
                        <Show when=move || characters.with(|summaries| summaries.is_empty())>
                            <CloudSignInHint message=move_tr!("hint-no-characters") />
                        </Show>
                        <div class="character-list">
                            <For
                                each=move || characters.get()
                                key=|summary| summary.id
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

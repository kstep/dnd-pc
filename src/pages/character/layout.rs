use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, nested_router::Outlet, params::Params};
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    BASE_URL,
    model::{CharacterIdentityStoreFields, CharacterStoreFields},
    rules::RulesRegistry,
    storage,
};

#[derive(Params, Clone, Debug, PartialEq)]
struct CharacterParams {
    id: Uuid,
}

#[component]
pub fn CharacterLayout() -> impl IntoView {
    let params = use_params::<CharacterParams>();

    let id = move || params.get().ok().map(|params| params.id);

    let character = move || id().and_then(|id| storage::load_character(&id));

    // If the character exists, render the layout; otherwise show not-found
    move || {
        if let Some(char_data) = character() {
            let store = Store::new(char_data);

            // Auto-save effect: track() subscribes to changes,
            // update_untracked() gives &mut without re-triggering.
            Effect::new(move || {
                store.track();
                store.update_untracked(storage::save_character);
            });

            // Fill labels and empty descriptions from registry definitions.
            // fill_from_registry also triggers fetches for missing definitions.
            let registry = expect_context::<RulesRegistry>();
            Effect::new(move || {
                store.update(|c| {
                    registry.fill_from_registry(c);
                });
            });

            // On locale change: clear all labels and descriptions so
            // fill_from_registry re-fills them from the new locale.
            let i18n = expect_context::<leptos_fluent::I18n>();
            let prev_lang = RwSignal::new(i18n.language.get_untracked().id.to_string());
            Effect::new(move || {
                let current = i18n.language.get().id.to_string();
                let prev = prev_lang.get_untracked();
                if current != prev {
                    prev_lang.set(current);
                    store.update(|c| {
                        c.clear_all_labels();
                    });
                }
            });

            // Reload store when cloud sync pulls a newer version.
            storage::track_cloud_character(
                store.read_untracked().id,
                move || store.read_untracked().updated_at,
                move |fresh| store.set(fresh),
            );

            // Provide context so child components can access the store
            provide_context(store);

            let name = Memo::new(move |_| store.identity().name().get());
            let class_summary = Memo::new(move |_| store.read().class_summary());
            let title = move || {
                let name = name.read();
                let summary = class_summary.read();
                match (name.is_empty(), summary.is_empty()) {
                    (true, true) => "D&D PC".to_string(),
                    (true, false) => summary.to_string(),
                    (false, true) => name.to_string(),
                    (false, false) => format!("{name} — {summary}"),
                }
            };

            Either::Left(view! {
                <Title text=title />
                <div class="character-sheet">
                    <Outlet />
                </div>
            })
        } else {
            Either::Right(view! {
                <div class="not-found">
                    <h1>{move_tr!("character-not-found")}</h1>
                    <A href=format!("{BASE_URL}/")>{move_tr!("back-to-list")}</A>
                </div>
            })
        }
    }
}

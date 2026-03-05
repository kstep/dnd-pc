use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, nested_router::Outlet, params::Params};
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    BASE_URL,
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields},
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

    move || {
        if let Some(char_data) = character() {
            Either::Left(view! { <CharacterInner char_data /> })
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

#[component]
fn CharacterInner(char_data: Character) -> impl IntoView {
    let store = Store::new(char_data);

    // Provide context first so child components can access the store.
    provide_context(store);

    // Auto-save + cloud sync pull (touch gated on initial sync).
    storage::setup_auto_save(store);

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
    let prev_lang = RwSignal::new(i18n.language.get_untracked().id);
    Effect::new(move || {
        let current = i18n.language.get().id;
        if current != prev_lang.get_untracked() {
            prev_lang.set(current);
            store.update(|c| {
                c.clear_all_labels();
            });
        }
    });

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

    view! {
        <Title text=title />
        <div class="character-sheet">
            <Outlet />
        </div>
    }
}

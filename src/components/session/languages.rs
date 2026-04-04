use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::model::{Character, CharacterStoreFields};

#[component]
pub fn LanguagesBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let languages = store.languages();
    let langs = move || {
        languages
            .read()
            .iter()
            .filter(|lang| !lang.is_empty())
            .cloned()
            .collect::<Vec<_>>()
    };

    move || {
        let langs = langs();

        if langs.is_empty() {
            None
        } else {
            Some(view! {
                <h4 class="session-subsection-title">{move_tr!("session-languages")}</h4>
                <p class="session-languages">{langs.join(", ")}</p>
            })
        }
    }
}

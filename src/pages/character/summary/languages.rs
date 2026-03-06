use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::model::{Character, CharacterStoreFields};

#[component]
pub fn LanguagesBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let langs = store
        .languages()
        .read()
        .iter()
        .filter(|lang| !lang.is_empty())
        .cloned()
        .collect::<Vec<_>>();

    if langs.is_empty() {
        return None;
    }

    Some(view! {
        <h4 class="summary-subsection-title">{move_tr!("summary-languages")}</h4>
        <p class="summary-languages">{langs.join(", ")}</p>
    })
}

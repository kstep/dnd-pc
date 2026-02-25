use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{components::*, path};

mod components;
mod model;
mod pages;
mod storage;

pub const BASE_URL: &str = match option_env!("BASE_URL") {
    Some(url) => url,
    None => "",
};

use pages::{character_list::CharacterList, character_sheet::CharacterSheet, not_found::NotFound};

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Html attr:lang="en" attr:dir="ltr" attr:data-theme="light" />
        <Title text="D&D 5e Character Sheet" />
        <Meta charset="UTF-8" />
        <Meta name="viewport" content="width=device-width, initial-scale=1.0" />

        <Router base=option_env!("BASE_URL").unwrap_or_default()>
            <Routes fallback=|| view! { <NotFound /> }>
                <Route path=path!("/") view=CharacterList />
                <Route path=path!("/character/:id") view=CharacterSheet />
            </Routes>
        </Router>
    }
}

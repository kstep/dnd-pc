use fluent_templates::static_loader;
use leptos::prelude::*;
use leptos_fluent::leptos_fluent;
use leptos_meta::*;
use leptos_router::{components::*, path};

mod components;
mod model;
mod pages;
pub mod rules;
mod share;
mod storage;

pub const BASE_URL: &str = match option_env!("BASE_URL") {
    Some(url) => url,
    None => "",
};

static_loader! {
    pub static TRANSLATIONS = {
        locales: "./locales",
        fallback_language: "en",
    };
}

use pages::{
    character_list::CharacterList, character_sheet::CharacterSheet,
    import_character::ImportCharacter, not_found::NotFound,
};
use rules::RulesRegistry;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    provide_context(RulesRegistry::new());

    view! {
        <Html attr:lang="en" attr:dir="ltr" attr:data-theme="light" />
        <Meta charset="UTF-8" />
        <Meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <Link rel="manifest" href=format!("{BASE_URL}/manifest.json") />
        <Link rel="apple-touch-icon" href=format!("{BASE_URL}/icons/icon-192.png") />

        <I18nProvider>
            <Router base=option_env!("BASE_URL").unwrap_or_default()>
                <Routes fallback=|| view! { <NotFound /> }>
                    <Route path=path!("/") view=CharacterList />
                    <Route path=path!("/c/:id") view=CharacterSheet />
                    <Route path=path!("/s/:data") view=ImportCharacter />
                </Routes>
            </Router>
        </I18nProvider>
    }
}

#[component]
fn I18nProvider(children: Children) -> impl IntoView {
    leptos_fluent! {
        children: children(),
        translations: [TRANSLATIONS],
        locales: "./locales",
        default_language: "en",
        set_language_to_local_storage: true,
        initial_language_from_local_storage: true,
        initial_language_from_navigator: true,
        sync_html_tag_lang: true,
    }
}

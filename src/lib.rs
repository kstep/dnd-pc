use fluent_templates::static_loader;
use leptos::prelude::*;
use leptos_fluent::leptos_fluent;
use leptos_meta::{Html, Link, Meta, provide_meta_context};
use leptos_router::{
    components::{ParentRoute, Route, Router, Routes},
    path,
};

mod components;
pub mod constvec;
mod firebase;
mod model;
mod pages;
pub mod rules;
mod share;
mod storage;
pub mod vecset;

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

use components::{language_switcher::LanguageSwitcher, sync_indicator::SyncIndicator};
use pages::{
    character_layout::CharacterLayout,
    character_list::CharacterList,
    character_sheet::CharacterSheet,
    character_summary::CharacterSummary,
    import_character::ImportCharacter,
    not_found::NotFound,
    reference::{
        background::BackgroundReference, class::ClassReference, race::RaceReference,
        spell::SpellReference,
    },
};
use rules::RulesRegistry;
use wasm_bindgen::JsCast;

/// Returns a reactive signal that tracks the current theme name.
/// Seeds from `window.matchMedia("(prefers-color-scheme: dark)")` and
/// updates in real time via a `change` event listener.
fn use_theme() -> ReadSignal<&'static str> {
    let mql = leptos::prelude::window()
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten();
    let theme = RwSignal::new(if mql.as_ref().map(|m| m.matches()).unwrap_or(false) {
        "dark"
    } else {
        "light"
    });
    if let Some(mql) = mql {
        let closure = wasm_bindgen::closure::Closure::<dyn Fn()>::new({
            let mql = mql.clone();
            move || theme.set(if mql.matches() { "dark" } else { "light" })
        });
        let _ = mql.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        // Leak the closure to keep the event listener alive for the entire app
        // lifetime.
        closure.forget();
    }
    theme.read_only()
}

#[component]
pub fn App() -> impl IntoView {
    provide_i18n_context();
    provide_meta_context();

    let theme = use_theme();
    let i18n = expect_context::<leptos_fluent::I18n>();
    provide_context(RulesRegistry::new(i18n));
    storage::init_sync();

    view! {
        <Html attr:lang="en" attr:dir="ltr" attr:data-theme=move || theme.get() />
        <Meta charset="UTF-8" />
        <Meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <Link rel="manifest" href=format!("{BASE_URL}/manifest.json") />
        <Link rel="apple-touch-icon" href=format!("{BASE_URL}/icons/icon-192.png") />

        <LanguageSwitcher />
        <SyncIndicator />
        <Router base=option_env!("BASE_URL").unwrap_or_default()>
            <Routes fallback=|| view! { <NotFound /> }>
                <Route path=path!("/") view=CharacterList />
                <ParentRoute path=path!("/c/:id") view=CharacterLayout>
                    <Route path=path!("") view=CharacterSheet />
                    <Route path=path!("/summary") view=CharacterSummary />
                </ParentRoute>
                <Route path=path!("/s/:data") view=ImportCharacter />
                <Route path=path!("/r/class") view=ClassReference />
                <Route path=path!("/r/class/:name") view=ClassReference />
                <Route path=path!("/r/class/:name/:subname") view=ClassReference />
                <Route path=path!("/r/race") view=RaceReference />
                <Route path=path!("/r/race/:name") view=RaceReference />
                <Route path=path!("/r/background") view=BackgroundReference />
                <Route path=path!("/r/background/:name") view=BackgroundReference />
                <Route path=path!("/r/spell") view=SpellReference />
                <Route path=path!("/r/spell/:list") view=SpellReference />
            </Routes>
        </Router>
    }
}

fn provide_i18n_context() {
    leptos_fluent! {
        translations: [TRANSLATIONS],
        locales: "./locales",
        default_language: "en",
        set_language_to_local_storage: true,
        initial_language_from_local_storage: true,
        initial_language_from_navigator: true,
        sync_html_tag_lang: true,
    };
}

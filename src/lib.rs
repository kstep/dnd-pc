use fluent_templates::static_loader;
use leptos::prelude::*;
use leptos_fluent::leptos_fluent;
use leptos_meta::{Html, provide_meta_context};
use leptos_router::{
    components::{ParentRoute, Route, Router, Routes},
    path,
};

mod components;
pub mod constvec;
mod demap;
mod effective;
mod expr;
mod firebase;
mod hooks;
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

use components::{
    logo::IsRouting,
    navbar::{ActiveCharacterId, Navbar, ViewClass},
};
use hooks::use_theme;
use pages::{
    character::{
        layout::CharacterLayout, list::CharacterList, sheet::CharacterSheet,
        summary::CharacterSummary,
    },
    import_character::{ImportCharacter, ImportCloudCharacter, ImportLayout},
    not_found::NotFound,
    reference::{
        ReferenceLayout, background::BackgroundReference, class::ClassReference,
        feature::FeatureReference, species::SpeciesReference, spell::SpellReference,
    },
};
use rules::RulesRegistry;

#[component]
pub fn App() -> impl IntoView {
    untrack(provide_i18n_context);
    provide_meta_context();

    let theme = use_theme();
    let i18n = expect_context::<leptos_fluent::I18n>();
    provide_context(RulesRegistry::new(i18n));
    provide_context(ActiveCharacterId::default());
    let view_class = ViewClass::default();
    provide_context(view_class);
    let is_routing = IsRouting::default();
    provide_context(is_routing);
    storage::init_sync();

    view! {
        <Html attr:lang="en" attr:dir="ltr" attr:data-theme=move || theme.get() attr:class=move || view_class.0.get() />

        <Router base=BASE_URL set_is_routing=is_routing.0>
            <Navbar />
            <main>
                <Routes fallback=|| view! { <NotFound /> }>
                    <Route path=path!("/") view=CharacterList />
                    <ParentRoute path=path!("/c/:id") view=CharacterLayout>
                        <Route path=path!("") view=CharacterSheet />
                        <Route path=path!("/summary") view=CharacterSummary />
                    </ParentRoute>
                    <ParentRoute path=path!("/s") view=ImportLayout>
                        <Route path=path!("/:user_id/:char_id") view=ImportCloudCharacter />
                        <Route path=path!("/:data") view=ImportCharacter />
                    </ParentRoute>
                    <ParentRoute path=path!("/r") view=ReferenceLayout>
                        <Route path=path!("/class") view=ClassReference />
                        <Route path=path!("/class/:name") view=ClassReference />
                        <Route path=path!("/class/:name/:subname") view=ClassReference />
                        <Route path=path!("/species") view=SpeciesReference />
                        <Route path=path!("/species/:name") view=SpeciesReference />
                        <Route path=path!("/background") view=BackgroundReference />
                        <Route path=path!("/background/:name") view=BackgroundReference />
                        <Route path=path!("/feature") view=FeatureReference />
                        <Route path=path!("/spell") view=SpellReference />
                        <Route path=path!("/spell/:list") view=SpellReference />
                    </ParentRoute>
                </Routes>
            </main>
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

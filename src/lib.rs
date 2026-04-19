use fluent_templates::static_loader;
use leptos::prelude::*;
use leptos_fluent::leptos_fluent;
use leptos_meta::{Html, provide_meta_context};
use leptos_router::{
    components::{ParentRoute, Redirect, Route, Router, Routes},
    hooks::use_params,
    params::Params,
    path,
};

mod ai;
mod components;
pub mod constvec;
mod demap;
mod effective;
mod export;
mod expr;
mod firebase;
mod hooks;
mod model;
mod names;
mod pages;
pub mod rules;
mod serde_util;
mod share;
mod storage;
pub mod vecset;

pub const BASE_URL: &str = match option_env!("BASE_URL") {
    Some(url) => url,
    None => "",
};

const BUILD_COMMIT: &str = env!("BUILD_COMMIT");
const BUILD_DATE: &str = env!("BUILD_DATE");

pub static LOGO_SVG: &str = include_str!("../public/icons/logo.svg");

static_loader! {
    pub static TRANSLATIONS = {
        locales: "./locales",
        fallback_language: "en",
    };
}

use components::{
    build_info::BuildInfo,
    logo::IsRouting,
    navbar::{ActiveCharacterId, Navbar},
    signin_toast::SignInToastTrigger,
    sync_error_toast::SyncErrorToastTrigger,
    toast::{ToastContainer, provide_toast_context},
};
use hooks::use_theme;
use pages::{
    character::{
        editor::CharacterEditor,
        layout::CharacterLayout,
        list::CharacterList,
        quick_start::QuickStart,
        session::CharacterSession,
        story::CharacterStory,
        tabs::{
            backstory::BackstoryTab, build::BuildTab, inventory::InventoryTab, magic::MagicTab,
            stats::StatsTab,
        },
    },
    import_character::{ImportCharacter, ImportCloudCharacter},
    not_found::NotFound,
    reference::{
        background::BackgroundReference, class::ClassReference, feature::FeatureReference,
        species::SpeciesReference, spell::SpellReference,
    },
};
use rules::RulesRegistry;

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct IdParam {
    id: uuid::Uuid,
}

#[component]
fn RedirectToLastTab() -> impl IntoView {
    let params = use_params::<IdParam>();
    move || {
        params.get().ok().map(|p| {
            let tab = storage::load_last_editor_tab();
            let path = format!("/c/{}/{tab}", p.id);
            view! { <Redirect path=path /> }
        })
    }
}

#[component]
pub fn App() -> impl IntoView {
    untrack(provide_i18n_context);
    provide_meta_context();

    let theme = use_theme();
    let i18n = expect_context::<leptos_fluent::I18n>();
    provide_context(RulesRegistry::new(i18n));
    provide_context(ActiveCharacterId::default());
    let is_routing = IsRouting::default();
    provide_context(is_routing);
    provide_toast_context();
    storage::init_sync();

    view! {
        <Html attr:lang="en" attr:dir="ltr" attr:data-theme=move || theme.get() />

        <Router base=BASE_URL set_is_routing=is_routing.0>
            <Navbar />
            <main>
                <Routes fallback=|| view! { <NotFound /> }>
                    <Route path=path!("/") view=CharacterList />
                    <ParentRoute path=path!("/c/:id") view=CharacterLayout>
                        <ParentRoute path=path!("") view=CharacterEditor>
                            <Route path=path!("") view=RedirectToLastTab />
                            <Route path=path!("/stats") view=StatsTab />
                            <Route path=path!("/build") view=BuildTab />
                            <Route path=path!("/magic") view=MagicTab />
                            <Route path=path!("/inventory") view=InventoryTab />
                            <Route path=path!("/backstory") view=BackstoryTab />
                        </ParentRoute>
                        <Route path=path!("/session") view=CharacterSession />
                        <Route path=path!("/quick-start") view=QuickStart />
                        <Route path=path!("/story") view=CharacterStory />
                        <Route path=path!("/story/:story_id") view=CharacterStory />
                    </ParentRoute>
                    <Route path=path!("/s/:user_id/:char_id") view=ImportCloudCharacter />
                    <Route path=path!("/s/:data") view=ImportCharacter />
                    <Route path=path!("/r/class") view=ClassReference />
                    <Route path=path!("/r/class/:name") view=ClassReference />
                    <Route path=path!("/r/class/:name/:subname") view=ClassReference />
                    <Route path=path!("/r/species") view=SpeciesReference />
                    <Route path=path!("/r/species/:name") view=SpeciesReference />
                    <Route path=path!("/r/background") view=BackgroundReference />
                    <Route path=path!("/r/background/:name") view=BackgroundReference />
                    <Route path=path!("/r/feature") view=FeatureReference />
                    <Route path=path!("/r/feature/:category") view=FeatureReference />
                    <Route path=path!("/r/spell") view=SpellReference />
                    <Route path=path!("/r/spell/:list") view=SpellReference />
                </Routes>
            </main>
        </Router>
        <ToastContainer />
        <SignInToastTrigger />
        <SyncErrorToastTrigger />
        <BuildInfo />
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

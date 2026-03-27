use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::{components::A, hooks::use_location};
use uuid::Uuid;

use crate::{
    BASE_URL,
    components::{
        icon::Icon, language_switcher::LanguageSwitcher, logo::Logo, sync_indicator::SyncIndicator,
    },
};

#[derive(Clone, Copy, Default)]
pub struct ActiveCharacterId(pub RwSignal<Option<Uuid>>);

#[component]
fn RefLinks() -> impl IntoView {
    view! {
        <A href=format!("{BASE_URL}/r/class") attr:class="navbar-link">
            {move_tr!("ref-classes")}
        </A>
        <A href=format!("{BASE_URL}/r/species") attr:class="navbar-link">
            {move_tr!("ref-species")}
        </A>
        <A href=format!("{BASE_URL}/r/background") attr:class="navbar-link">
            {move_tr!("ref-backgrounds")}
        </A>
        <A href=format!("{BASE_URL}/r/spell") attr:class="navbar-link">
            {move_tr!("ref-spells")}
        </A>
        <A href=format!("{BASE_URL}/r/feature") attr:class="navbar-link">
            {move_tr!("ref-features")}
        </A>
    }
}

#[component]
pub fn Navbar() -> impl IntoView {
    let active_id = expect_context::<ActiveCharacterId>().0;
    let i18n = expect_context::<leptos_fluent::I18n>();
    let ref_open = RwSignal::new(false);
    let location = use_location();
    let ref_key = move || {
        let path = location.pathname.read();
        let section = path
            .strip_prefix(BASE_URL)
            .and_then(|rest| rest.strip_prefix("/r/"))
            .and_then(|rest| rest.split('/').next())
            .unwrap_or("");
        match section {
            "class" => Some("ref-classes"),
            "species" => Some("ref-species"),
            "background" => Some("ref-backgrounds"),
            "spell" => Some("ref-spells"),
            "feature" => Some("ref-features"),
            _ => None,
        }
    };
    let on_ref_page = move || ref_key().is_some();
    let current_ref_page = move || ref_key().map(|key| i18n.tr(key)).unwrap_or_default();

    view! {
        <nav class="navbar">
            <div class="navbar-left">
                <A href=format!("{BASE_URL}/") attr:class="navbar-brand">
                    <Logo />
                    <span class="navbar-title">{move_tr!("page-characters")}</span>
                </A>
                {move || active_id.get().map(|id| view! {
                    <div class="navbar-links">
                        <A href=format!("{BASE_URL}/c/{id}") exact=true attr:class="navbar-link">
                            {move_tr!("view-full-sheet")}
                        </A>
                        <A href=format!("{BASE_URL}/c/{id}/summary") exact=true attr:class="navbar-link">
                            {move_tr!("view-summary")}
                        </A>
                    </div>
                })}
                <div class="navbar-ref">
                    <button
                        class="navbar-link navbar-ref-toggle"
                        on:click=move |_| ref_open.update(|v| *v = !*v)
                    >
                        <Icon name="scroll-text" size=16 />
                        <span class="navbar-ref-label">{move_tr!("ref-reference")}</span>
                        <span class="navbar-ref-current">{current_ref_page}</span>
                    </button>
                    <Show when=move || ref_open.get()>
                        <div class="navbar-ref-dropdown" on:click=move |_| ref_open.set(false)>
                            <RefLinks />
                        </div>
                    </Show>
                </div>
                <Show when=on_ref_page>
                    <div class="navbar-links navbar-ref-inline">
                        <RefLinks />
                    </div>
                </Show>
            </div>
            <div class="navbar-right">
                <SyncIndicator />
                <LanguageSwitcher />
            </div>
        </nav>
    }
}

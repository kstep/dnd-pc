use leptos::prelude::*;

use crate::{components::icon::Icon, hooks::use_hash_href};

const SECTIONS: &[(&str, &str)] = &[
    ("session-actions", "swords"),
    ("session-stats", "scroll-text"),
    ("session-resources", "wand"),
    ("session-effects", "zap"),
    ("session-backpack", "backpack"),
];

#[component]
pub fn SessionNav() -> impl IntoView {
    let i18n = expect_context::<leptos_fluent::I18n>();
    let hash_href = use_hash_href();

    let items = SECTIONS
        .iter()
        .map(|&(section_id, icon_name)| {
            let href = hash_href(section_id);
            let label = move || i18n.tr(section_id);
            view! {
                <a class="floating-nav-btn" href=href title=label rel="external">
                    <Icon name=icon_name size=18 />
                </a>
            }
        })
        .collect_view();

    view! {
        <nav class="floating-nav">
            {items}
        </nav>
    }
}

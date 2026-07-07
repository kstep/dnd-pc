use std::time::Duration;

use leptos::{leptos_dom::helpers::set_timeout, prelude::*};

#[component]
pub fn LanguageSwitcher() -> impl IntoView {
    let i18n = expect_context::<leptos_fluent::I18n>();

    // Optimistic mirror of `i18n.language` — flips on click before the heavy
    // cascade runs, so the active-button visual updates on the next paint.
    let pending = RwSignal::new(i18n.language.get_untracked());
    Effect::new(move || pending.set(i18n.language.get()));

    view! {
        <div class="lang-switcher">
            {i18n
                .languages
                .iter()
                .map(|lang| {
                    let lang = *lang;
                    let is_active = move || pending.get() == lang;
                    view! {
                        <button
                            class="lang-btn"
                            class:active=is_active
                            on:click=move |_| {
                                pending.set(lang);
                                set_timeout(move || i18n.language.set(lang), Duration::ZERO);
                            }
                        >
                            {lang.id.to_string().to_uppercase()}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

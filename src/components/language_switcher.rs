use leptos::prelude::*;

#[component]
pub fn LanguageSwitcher() -> impl IntoView {
    let i18n = expect_context::<leptos_fluent::I18n>();

    view! {
        <div class="lang-switcher">
            {i18n.languages.iter().map(|lang| {
                let lang = *lang;
                let is_active = move || i18n.language.get() == lang;
                view! {
                    <button
                        class="lang-btn"
                        class:active=is_active
                        on:click=move |_| {
                            i18n.language.set(lang);
                        }
                    >
                        {lang.id.to_string().to_uppercase()}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

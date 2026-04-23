use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::BASE_URL;

#[component]
pub fn Ref(
    #[prop(into)] href: Signal<String>,
    #[prop(optional)] exact: bool,
    #[prop(default = true)] scroll: bool,
    children: Children,
) -> impl IntoView {
    let full_target = Memo::new(move |_| {
        let h = href.get();
        if h.starts_with('/') && !h.starts_with("//") {
            format!("{BASE_URL}{h}")
        } else {
            h
        }
    });

    let pathname = use_location().pathname;
    let is_active = move || {
        pathname.with(|loc| {
            full_target.with(|target| {
                if exact {
                    loc == target
                } else {
                    loc == target
                        || (loc.starts_with(target.as_str())
                            && loc[target.len()..].starts_with('/'))
                }
            })
        })
    };

    view! {
        <a
            href=move || full_target.get()
            aria-current=move || is_active().then_some("page")
            data-noscroll=!scroll
        >
            {children()}
        </a>
    }
}

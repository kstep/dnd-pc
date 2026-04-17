use leptos::prelude::*;

/// `<span class="entry-name">` that toggles `.expanded` on the nearest
/// `.entry-item` when clicked, unless the item's toggle button is disabled.
#[component]
pub fn EntryName(children: Children) -> impl IntoView {
    view! {
        <span
            class="entry-name"
            on:click=move |e| {
                let span: web_sys::HtmlElement = event_target(&e);
                let Ok(Some(entry)) = span.closest(".entry-item") else { return };
                if entry
                    .query_selector(":scope > .btn-toggle-desc[disabled]")
                    .ok()
                    .flatten()
                    .is_some()
                {
                    return;
                }
                let _ = entry.class_list().toggle("expanded");
            }
        >
            {children()}
        </span>
    }
}

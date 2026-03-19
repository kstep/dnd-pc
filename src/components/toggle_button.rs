use leptos::prelude::*;

/// Toggle button that expands/collapses the nearest `.entry-item` parent
/// by toggling the `.expanded` class on it. Icon switches via CSS.
#[component]
pub fn ToggleButton() -> impl IntoView {
    view! {
        <button
            class="btn-toggle-desc"
            on:click=move |e| {
                let btn: web_sys::HtmlElement = event_target(&e);
                if let Ok(Some(entry)) = btn.closest(".entry-item") {
                    let _ = entry.class_list().toggle("expanded");
                }
            }
        />
    }
}

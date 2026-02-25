use gloo_storage::{LocalStorage, Storage};
use leptos::{html, prelude::*};

fn panel_key(class: &str) -> String {
    format!("dnd_pc_panel_{class}")
}

#[component]
pub fn Panel(
    #[prop(into)] title: String,
    #[prop(into)] class: String,
    children: Children,
) -> impl IntoView {
    let key = panel_key(&class);
    let open = LocalStorage::get(&key).unwrap_or(true);
    let class = format!("panel {class}");
    let details_ref = NodeRef::<html::Details>::new();

    let on_toggle = move |_: web_sys::Event| {
        if let Some(el) = details_ref.get() {
            let _ = LocalStorage::set(&key, el.open());
        }
    };

    view! {
        <details class=class node_ref=details_ref open=open on:toggle=on_toggle>
            <summary>{title}</summary>
            {children()}
        </details>
    }
}

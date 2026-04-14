use leptos::{html, prelude::*};
use leptos_router::{components::A, hooks::use_location};
use wasm_bindgen::JsCast;

use crate::components::icon::Icon;

#[derive(Clone)]
pub struct TabItem {
    pub path: &'static str,
    pub label: Signal<String>,
    pub icon: &'static str,
    pub visible: Signal<bool>,
    pub has_updates: Signal<bool>,
}

impl TabItem {
    pub fn new(path: &'static str, label: Signal<String>, icon: &'static str) -> Self {
        Self {
            path,
            label,
            icon,
            visible: Signal::stored(true),
            has_updates: Signal::stored(false),
        }
    }

    pub fn visible_when(mut self, visible: Signal<bool>) -> Self {
        self.visible = visible;
        self
    }

    pub fn marked_when(mut self, has_updates: Signal<bool>) -> Self {
        self.has_updates = has_updates;
        self
    }
}

#[component]
pub fn TabNav(#[prop(into)] base: Signal<String>, items: Vec<TabItem>) -> impl IntoView {
    let nav_ref = NodeRef::<html::Nav>::new();
    let indicator_style = RwSignal::new(String::from("opacity: 0"));
    let location = use_location();

    Effect::new(move |_| {
        let _ = location.pathname.read();
        let Some(nav) = nav_ref.get() else {
            return;
        };
        request_animation_frame(move || {
            let Ok(Some(active)) = nav.query_selector("a[aria-current=\"page\"]") else {
                indicator_style.set("opacity: 0".into());
                return;
            };
            let Ok(active) = active.dyn_into::<web_sys::HtmlElement>() else {
                return;
            };
            indicator_style.set(format!(
                "left: {}px; width: {}px",
                active.offset_left(),
                active.offset_width()
            ));
        });
    });

    view! {
        <nav class="tab-nav" node_ref=nav_ref>
            {items
                .into_iter()
                .map(|item| {
                    let TabItem { path, label, icon, visible, has_updates } = item;
                    let class = move || {
                        if has_updates.get() {
                            "tab-nav-link has-updates"
                        } else {
                            "tab-nav-link"
                        }
                    };
                    view! {
                        <Show when=move || visible.get()>
                            <A
                                href=move || format!("{}/{}", base.get(), path)
                                scroll=false
                                attr:class=class
                            >
                                <Icon name=icon size=16 />
                                <span class="tab-nav-label">{label}</span>
                            </A>
                        </Show>
                    }
                })
                .collect_view()}
            <span class="tab-nav-indicator" style=indicator_style />
        </nav>
    }
}

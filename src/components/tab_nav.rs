use leptos::prelude::*;

use crate::components::{icon::Icon, ref_link::Ref};

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
    view! {
        <nav class="tab-nav">
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
                            <Ref
                                href=Signal::derive(move || format!("{}/{}", base.get(), path))
                                scroll=false
                                attr:class=class
                            >
                                <Icon name=icon size=16 />
                                <span class="tab-nav-label">{label}</span>
                            </Ref>
                        </Show>
                    }
                })
                .collect_view()}
        </nav>
    }
}

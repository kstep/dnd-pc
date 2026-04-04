use leptos::{either::Either, prelude::*};

use crate::components::toggle_button::ToggleButton;

pub struct SessionListItem {
    pub name: String,
    pub description: String,
    pub badge: Option<AnyView>,
}

#[component]
pub fn SessionList(items: Vec<SessionListItem>) -> impl IntoView {
    view! {
        <div class="entry-list">
            {items
                .into_iter()
                .map(|item| {
                    let has_desc = !item.description.is_empty();
                    view! {
                        <div class="entry-item">
                            {if has_desc {
                                Either::Left(view! { <ToggleButton /> })
                            } else {
                                Either::Right(view! {
                                    <button class="btn-toggle-desc" disabled=true />
                                })
                            }}
                            <div class="entry-content">
                                <span class="entry-name">{item.name}</span>
                                {item.badge}
                            </div>
                            <div class="entry-actions" />
                            {has_desc.then(|| view! {
                                <p class="entry-desc">{item.description.clone()}</p>
                            })}
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

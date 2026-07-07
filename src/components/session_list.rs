use leptos::{either::Either, prelude::*};

use crate::components::{entry_name::EntryName, markdown::Markdown, toggle_button::ToggleButton};

pub struct SessionListItem {
    pub name: String,
    pub description: String,
    pub badge: Option<AnyView>,
    pub actions: Option<AnyView>,
    /// Optional view rendered before the name inside `entry-content`. Used
    /// for the equipped checkbox on gear rows.
    pub name_prefix: Option<AnyView>,
    /// Optional view rendered between badges and actions, visually trailing
    /// the name row. Used for inline controls like quantity inputs that
    /// belong with the item's content, not its actions.
    pub name_extra: Option<AnyView>,
    /// Optional override for the description rendering. When `Some`, this
    /// view replaces the Markdown rendering of `description`. Used for
    /// editable descriptions (backpack) etc.
    pub description_view: Option<AnyView>,
}

#[component]
pub fn SessionList(items: Vec<SessionListItem>) -> impl IntoView {
    view! {
        <div class="entry-list">
            {items
                .into_iter()
                .map(|item| {
                    let has_desc = !item.description.is_empty() || item.description_view.is_some();
                    let description_content = item
                        .description_view
                        .or_else(|| {
                            (!item.description.is_empty())
                                .then(|| {
                                    view! { <Markdown text=item.description.clone() /> }.into_any()
                                })
                        });
                    let description = description_content
                        .map(|content| {
                            view! { <div class="entry-desc">{content}</div> }
                        });
                    view! {
                        <div class="entry-item">
                            {if has_desc {
                                Either::Left(view! { <ToggleButton /> })
                            } else {
                                Either::Right(
                                    view! { <button class="btn-toggle-desc" disabled=true /> },
                                )
                            }}
                            <div class="entry-content">
                                {item.name_prefix} <EntryName>{item.name}</EntryName>
                            </div>
                            {item
                                .badge
                                .map(|badges| view! { <div class="entry-badges">{badges}</div> })}
                            <div class="entry-actions">{item.name_extra} {item.actions}</div>
                            {description}
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

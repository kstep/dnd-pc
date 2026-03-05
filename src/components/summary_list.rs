use std::collections::HashSet;

use leptos::{either::Either, prelude::*};

use crate::components::{icon::Icon, toggle_button::ToggleButton};

pub struct SummaryListItem {
    pub name: String,
    pub description: String,
    pub badge: Option<AnyView>,
}

#[component]
pub fn SummaryList(items: Vec<SummaryListItem>) -> impl IntoView {
    let expanded = RwSignal::new(HashSet::<usize>::new());
    view! {
        <div class="summary-list">
            {items
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    let is_open = Signal::derive(move || expanded.get().contains(&i));
                    let has_desc = !item.description.is_empty();
                    view! {
                        <div class="summary-list-entry">
                            <div class="summary-list-row">
                                {if has_desc {
                                    Either::Left(view! {
                                        <ToggleButton
                                            expanded=is_open
                                            on_toggle=move || {
                                                expanded.update(|set| {
                                                    if !set.remove(&i) {
                                                        set.insert(i);
                                                    }
                                                })
                                            }
                                        />
                                    })
                                } else {
                                    Either::Right(view! {
                                        <button class="btn-toggle-desc" disabled=true>
                                            <Icon name="minus" />
                                        </button>
                                    })
                                }}
                                <span class="summary-list-name">{item.name}</span>
                                {item.badge}
                            </div>
                            <Show when=move || is_open.get() && has_desc>
                                <p class="summary-list-desc">{item.description.clone()}</p>
                            </Show>
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

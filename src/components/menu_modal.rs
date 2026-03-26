use leptos::prelude::*;

use crate::components::modal::Modal;

/// A menu item with a label and an optional secondary label.
#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub detail: String,
}

/// A modal that presents a list of choices and calls `on_select` with the
/// chosen index.
#[component]
pub fn MenuModal(
    show: RwSignal<bool>,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] items: Signal<Vec<MenuItem>>,
    #[prop(into)] on_select: Callback<usize>,
) -> impl IntoView {
    view! {
        <Modal show title>
            <div class="menu-modal-choices">
                {move || items.get().into_iter().enumerate().map(|(idx, item)| {
                    view! {
                        <button
                            class="menu-modal-choice"
                            on:click=move |_| {
                                show.set(false);
                                on_select.run(idx);
                            }
                        >
                            <span>{item.label}</span>
                            <span class="menu-modal-detail">{item.detail}</span>
                        </button>
                    }
                }).collect_view()}
            </div>
        </Modal>
    }
}

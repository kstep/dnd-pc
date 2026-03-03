use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::components::A;

use crate::{BASE_URL, components::icon::Icon};

#[component]
pub fn ReferenceSidebar(current_label: Signal<String>, children: ChildrenFn) -> impl IntoView {
    let manually_open = RwSignal::new(false);

    // Reset collapsed state when navigating to a different item
    Effect::new(move || {
        current_label.track();
        manually_open.set(false);
    });

    // Auto-expand when no current item is selected
    let open = Signal::derive(move || current_label.read().is_empty() || manually_open.get());

    view! {
        <aside class="reference-sidebar" class:open=move || open.get()>
            <A href=format!("{BASE_URL}/") attr:class="reference-home-link">
                {"\u{2190} "}{move_tr!("ref-home")}
            </A>
            {move || (!current_label.read().is_empty()).then(|| view! {
                <button
                    class="reference-nav-toggle"
                    on:click=move |_| manually_open.update(|v| *v = !*v)
                >
                    {move || current_label.get()}
                    <Icon name="chevron-down" size=14 />
                </button>
            })}
            {move || children()}
        </aside>
    }
}

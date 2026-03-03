use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::components::A;

use crate::{BASE_URL, components::icon::Icon};

#[component]
pub fn ReferenceSidebar(current_label: Signal<String>, children: ChildrenFn) -> impl IntoView {
    let open = RwSignal::new(false);

    view! {
        <aside class="reference-sidebar" class:open=move || open.get()>
            <A href=format!("{BASE_URL}/") attr:class="reference-home-link">
                {"\u{2190} "}{move_tr!("ref-home")}
            </A>
            <button
                class="reference-nav-toggle"
                on:click=move |_| open.update(|v| *v = !*v)
            >
                {move || current_label.get()}
                <Icon name="chevron-down" size=14 />
            </button>
            {move || children()}
        </aside>
    }
}

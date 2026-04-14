use leptos::prelude::*;

use crate::components::icon::Icon;

#[component]
pub fn SlotBox(
    #[prop(into)] label: Signal<String>,
    #[prop(default = false)] label_after: bool,
    #[prop(optional)] icon: Option<&'static str>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="slot-box" class:label-after=label_after>
            <span class="slot-box-label">
                {icon.map(|name| view! { <Icon name=name /> })}
                {label}
            </span>
            {children.map(|c| view! { <span class="slot-box-value">{c()}</span> })}
        </div>
    }
}

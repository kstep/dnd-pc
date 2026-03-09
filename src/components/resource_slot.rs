use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::components::icon::Icon;

#[component]
pub fn ResourceSlot(
    #[prop(into)] label: String,
    max: u32,
    used: u32,
    on_change: impl Fn(u32) + 'static + Send + Sync,
) -> impl IntoView {
    let remaining = max.saturating_sub(used);
    let all_used = used >= max;
    let on_change = StoredValue::new(on_change);
    view! {
        <div class="summary-slot">
            <span class="summary-slot-level">{label}</span>
            <span class="summary-slot-value">{remaining} " / " {max}</span>
            <button
                class="btn-icon"
                title=move_tr!("spend")
                disabled=all_used
                on:click=move |_| {
                    if used < max {
                        on_change.with_value(|f| f(used + 1));
                    }
                }
            >
                <Icon name="wand" size=14 />
            </button>
        </div>
    }
}

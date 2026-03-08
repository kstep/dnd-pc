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
    let max_str = max.to_string();
    let used_str = used.to_string();
    let all_used = used >= max;
    let on_change = StoredValue::new(on_change);
    view! {
        <div class="summary-slot">
            <span class="summary-slot-level">{label}</span>
            <input
                type="number"
                class="short-input"
                min="0"
                prop:max=max_str
                prop:value=used_str
                on:input=move |event| {
                    if let Ok(value) = event_target_value(&event).parse::<u32>() {
                        on_change.with_value(|f| f(value.min(max)));
                    }
                }
            />
            <span>"/" {max}</span>
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
                <Icon name="minus" size=14 />
            </button>
        </div>
    }
}

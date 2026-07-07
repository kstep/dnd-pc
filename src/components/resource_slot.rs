use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::components::{icon::Icon, slot_box::SlotBox};

#[component]
pub fn ResourceSlot(
    #[prop(into)] label: String,
    max: u32,
    used: u32,
    on_change: impl Fn(u32) + 'static + Send + Sync,
) -> impl IntoView {
    let remaining = RwSignal::new(max.saturating_sub(used));
    let on_change = StoredValue::new(on_change);
    let set_remaining = move |new_remaining: u32| {
        let clamped = new_remaining.min(max);
        remaining.set(clamped);
        on_change.with_value(|f| f(max - clamped));
    };
    view! {
        <SlotBox label=label>
            <input
                type="number"
                min=0
                max=max
                prop:value=move || remaining.get()
                on:change=move |event| {
                    let value = event_target_value(&event).parse().unwrap_or(remaining.get());
                    set_remaining(value);
                }
            />
            " / "
            {max}
            <button
                class="btn-icon"
                title=move_tr!("spend")
                disabled=move || remaining.get() == 0
                on:click=move |_| {
                    let current = remaining.get();
                    if current > 0 {
                        set_remaining(current - 1);
                    }
                }
            >
                <Icon name="wand" />
            </button>
        </SlotBox>
    }
}

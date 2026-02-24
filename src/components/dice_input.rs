use leptos::prelude::*;

use crate::model::Dice;

#[component]
pub fn DiceInput(value: Memo<Dice>, on_change: impl Fn(Dice) + Copy + 'static) -> impl IntoView {
    let count = move || value.get().count;
    let sides = move || value.get().sides;
    let modifier = move || value.get().modifier;

    view! {
        <div class="dice-input">
            <input
                type="number"
                class="dice-count"
                min="0"
                prop:value=move || count().to_string()
                on:input=move |e| {
                    if let Ok(v) = event_target_value(&e).parse::<u16>() {
                        let d = value.get();
                        on_change(Dice { count: v, ..d });
                    }
                }
            />
            <span class="dice-d">"d"</span>
            <input
                type="number"
                class="dice-sides"
                min="1"
                prop:value=move || sides().to_string()
                on:input=move |e| {
                    if let Ok(v) = event_target_value(&e).parse::<u16>() {
                        let d = value.get();
                        on_change(Dice { sides: v, ..d });
                    }
                }
            />
            <span class="dice-plus">"+"</span>
            <input
                type="number"
                class="dice-modifier"
                prop:value=move || modifier().to_string()
                on:input=move |e| {
                    if let Ok(v) = event_target_value(&e).parse::<i16>() {
                        let d = value.get();
                        on_change(Dice { modifier: v, ..d });
                    }
                }
            />
        </div>
    }
}

use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::components::icon::Icon;

#[component]
pub fn CastButton(
    #[prop(default = false)] disabled: bool,
    on_cast: impl Fn() + 'static + Send + Sync,
    /// Available spell slots: (level, remaining_count). Empty = no picker.
    #[prop(optional)]
    slots: Vec<(u32, u32)>,
    /// Spell's natural level — highlighted in picker.
    #[prop(default = 0)]
    spell_level: u32,
    /// Callback when a slot level is picked. Receives chosen level (1-9).
    #[prop(optional)]
    on_slot_cast: Option<Callback<u32>>,
) -> impl IntoView {
    let has_slots = !slots.is_empty() && on_slot_cast.is_some();
    let picker_open = RwSignal::new(false);
    let slots = StoredValue::new(slots);
    let on_slot_cast = StoredValue::new(on_slot_cast);

    let on_click = move |_| {
        if !has_slots {
            on_cast();
            return;
        }
        slots.with_value(|s| {
            if s.len() == 1 {
                on_slot_cast.with_value(|cb| {
                    if let Some(cb) = cb {
                        cb.run(s[0].0);
                    }
                });
            } else {
                picker_open.update(|v| *v = !*v);
            }
        });
    };

    view! {
        <span class="cast-btn-wrapper">
            <button
                class="btn-icon"
                title=move_tr!("cast")
                disabled=disabled
                on:click=on_click
            >
                <Icon name="wand" size=14 />
            </button>
            {has_slots.then(move || {
                view! {
                    <Show when=move || picker_open.get()>
                        <div class="cast-slot-picker">
                            {slots.with_value(|s| {
                                s.iter().map(|&(level, remaining)| {
                                    view! {
                                        <button
                                            class="cast-slot-pill"
                                            class:natural-level=level == spell_level
                                            on:click=move |_| {
                                                on_slot_cast.with_value(|cb| {
                                                    if let Some(cb) = cb {
                                                        cb.run(level);
                                                    }
                                                });
                                                picker_open.set(false);
                                            }
                                        >
                                            {level}
                                            <sub class="slot-remaining">{remaining}</sub>
                                        </button>
                                    }
                                }).collect_view()
                            })}
                            <button
                                class="btn-icon"
                                on:click=move |_| picker_open.set(false)
                            >
                                "✕"
                            </button>
                        </div>
                    </Show>
                }
            })}
        </span>
    }
}

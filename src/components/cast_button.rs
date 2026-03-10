use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::components::icon::Icon;

/// A cast option in the picker.
/// - `id`: 0 = free use, 1-9 = spell slot level, 100+ = points cost
/// - `label`: display text (e.g. "🎁", "3", "4 SP")
/// - `sublabel`: optional subscript (e.g. remaining count)
/// - `highlight`: whether to accent this option (e.g. natural spell level)
pub struct CastOption {
    pub id: u32,
    pub label: String,
    pub sublabel: Option<String>,
    pub highlight: bool,
}

#[component]
pub fn CastButton(
    #[prop(default = false)] disabled: bool,
    /// Cast options to show in picker. If exactly 1, auto-casts on click.
    #[prop(optional)]
    options: Vec<CastOption>,
    /// Callback when a cast option is picked. Receives the option id.
    on_cast: Callback<u32>,
) -> impl IntoView {
    let picker_open = RwSignal::new(false);
    let option_count = options.len();
    let options = StoredValue::new(options);
    let on_cast = StoredValue::new(on_cast);

    let on_click = move |_| {
        if option_count <= 1 {
            // 0 options = direct cast (simple button), 1 option = auto-pick
            let id = if option_count == 1 {
                options.with_value(|opts| opts[0].id)
            } else {
                0
            };
            on_cast.with_value(|callback| callback.run(id));
        } else {
            picker_open.update(|open| *open = !*open);
        }
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
            {(option_count > 1).then(move || {
                view! {
                    <Show when=move || picker_open.get()>
                        <div class="cast-slot-picker">
                            {options.with_value(|opts| {
                                opts.iter().map(|opt| {
                                    let id = opt.id;
                                    let highlight = opt.highlight;
                                    let label = opt.label.clone();
                                    let sublabel = opt.sublabel.clone();
                                    view! {
                                        <button
                                            class="cast-slot-pill"
                                            class:natural-level=highlight
                                            on:click=move |_| {
                                                on_cast.with_value(|callback| callback.run(id));
                                                picker_open.set(false);
                                            }
                                        >
                                            {label.clone()}
                                            {sublabel.clone().map(|s| view! {
                                                <sub class="slot-remaining">{s}</sub>
                                            })}
                                        </button>
                                    }
                                }).collect_view()
                            })}
                            <button
                                class="btn-icon"
                                on:click=move |_| picker_open.set(false)
                            >
                                <Icon name="x" size=14 />
                            </button>
                        </div>
                    </Show>
                }
            })}
        </span>
    }
}

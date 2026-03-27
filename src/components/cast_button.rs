use leptos::{either::EitherOf4, prelude::*};
use leptos_fluent::move_tr;

use crate::components::icon::Icon;

/// A cast option in the spell picker.
#[derive(Clone)]
pub enum CastOption {
    /// Free use (racial/feature innate cast). Shows gift icon.
    FreeUse { available: u32, max: u32 },
    /// Spend points (e.g. sorcery points). Shows cost with suffix like "4 SP".
    PointsCost { cost: u32, suffix: String },
    /// Use a spell slot. Shows level number with remaining count.
    SpellSlot {
        level: u32,
        remaining: u32,
        natural: bool,
    },
    /// Ritual cast. No slot consumed, uses the spell's native level.
    Ritual { level: u32 },
}

impl CastOption {
    fn is_natural(&self) -> bool {
        matches!(self, CastOption::SpellSlot { natural: true, .. })
    }

    fn view(self) -> impl IntoView {
        match self {
            CastOption::FreeUse { available, max } => EitherOf4::A(view! {
                <Icon name="gift" size=14 />
                <sub class="slot-remaining">{available}"/"{max}</sub>
            }),
            CastOption::PointsCost { cost, suffix } => EitherOf4::B(view! {
                {cost}" "{suffix}
            }),
            CastOption::SpellSlot {
                level, remaining, ..
            } => EitherOf4::C(view! {
                {level}
                <sub class="slot-remaining">{remaining}</sub>
            }),
            CastOption::Ritual { .. } => EitherOf4::D(view! {
                <Icon name="book-open" size=14 />
            }),
        }
    }
}

#[component]
pub fn CastButton(
    #[prop(default = false)] disabled: bool,
    /// Cast options to show in picker. If exactly 1, auto-casts on click.
    #[prop(optional)]
    options: Vec<CastOption>,
    /// Callback when a cast option is picked. Receives the option discriminant.
    on_cast: Callback<CastOption>,
) -> impl IntoView {
    let picker_open = RwSignal::new(false);
    let option_count = options.len();
    let options = StoredValue::new(options);
    let on_cast = StoredValue::new(on_cast);

    let on_click = move |_| {
        if option_count <= 1 {
            // 0 options = direct cast (simple button), 1 option = auto-pick
            let opt = if option_count == 1 {
                options.with_value(|opts| opts[0].clone())
            } else {
                CastOption::FreeUse {
                    available: 0,
                    max: 0,
                }
            };
            on_cast.with_value(|callback| callback.run(opt));
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
                                    let highlight = opt.is_natural();
                                    let opt_clone = opt.clone();
                                    let opt_view = opt.clone().view();
                                    view! {
                                        <button
                                            class="cast-slot-pill"
                                            class:natural-level=highlight
                                            on:click=move |_| {
                                                on_cast.with_value(|callback| callback.run(opt_clone.clone()));
                                                picker_open.set(false);
                                            }
                                        >
                                            {opt_view}
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

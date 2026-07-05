use leptos::{either::EitherOf4, prelude::*};
use leptos_fluent::{I18n, move_tr};

use crate::{
    components::icon::Icon,
    model::{SpellSlotPool, SpellSlots, Translatable},
};

/// A cast option in the spell picker.
#[derive(Clone)]
pub enum CastOption {
    /// Free use (racial/feature innate cast). Shows gift icon.
    FreeUse { available: u32, max: u32 },
    /// Spend points (e.g. sorcery points). Shows cost with suffix like "4 SP".
    PointsCost { cost: u32, suffix: String },
    /// Use a spell slot. Shows level number with remaining count.
    SpellSlot {
        pool: SpellSlotPool,
        level: u32,
        remaining: u32,
        natural: bool,
    },
    /// Ritual cast. No slot consumed, uses the spell's native level.
    Ritual { level: u32 },
}

impl CastOption {
    /// Slot options able to pay for a spell of `spell_level`: native pool
    /// first (levels ascending), then the other pool; only slots with
    /// remaining uses. `natural` = native pool AND level == spell_level.
    pub fn slot_options(
        slots: &SpellSlots,
        spell_level: u32,
        native_pool: SpellSlotPool,
    ) -> impl Iterator<Item = CastOption> + '_ {
        let other_pool = match native_pool {
            SpellSlotPool::Arcane => SpellSlotPool::Pact,
            SpellSlotPool::Pact => SpellSlotPool::Arcane,
        };
        [native_pool, other_pool].into_iter().flat_map(move |pool| {
            slots
                .iter_pool(pool)
                .filter(move |(level, slot)| *level >= spell_level && slot.available() > 0)
                .map(move |(level, slot)| CastOption::SpellSlot {
                    pool,
                    level,
                    remaining: slot.available(),
                    natural: pool == native_pool && level == spell_level,
                })
        })
    }

    fn slot_pool(&self) -> Option<SpellSlotPool> {
        match self {
            CastOption::SpellSlot { pool, .. } => Some(*pool),
            _ => None,
        }
    }

    fn is_natural(&self) -> bool {
        matches!(self, CastOption::SpellSlot { natural: true, .. })
    }

    fn view(self, i18n: I18n, foreign: bool) -> impl IntoView {
        match self {
            CastOption::FreeUse { available, max } => EitherOf4::A(view! {
                <Icon name="gift" />
                <sub class="slot-remaining">{available}"/"{max}</sub>
            }),
            CastOption::PointsCost { cost, suffix } => EitherOf4::B(view! {
                {cost}" "{suffix}
            }),
            CastOption::SpellSlot {
                pool,
                level,
                remaining,
                ..
            } => EitherOf4::C(view! {
                {level}
                <sub class="slot-remaining">{remaining}</sub>
                {foreign.then(|| {
                    let label = Signal::derive(move || i18n.tr(pool.tr_key()));
                    view! { <sub class="slot-pool">{label}</sub> }
                })}
            }),
            CastOption::Ritual { .. } => EitherOf4::D(view! {
                <Icon name="book-open" />
            }),
        }
    }
}

#[component]
pub fn CastButton(
    #[prop(default = false)] disabled: bool,
    /// Cast options to show in picker. If exactly 1, auto-casts on click.
    /// Slot options must be grouped by pool, the casting feature's own pool
    /// first — foreign-pool labeling derives from the first slot option.
    #[prop(optional)]
    options: Vec<CastOption>,
    /// Callback when a cast option is picked. Receives the option discriminant.
    on_cast: Callback<CastOption>,
) -> impl IntoView {
    let i18n = expect_context::<I18n>();
    let picker_open = RwSignal::new(false);
    let option_count = options.len();
    let native_slot_pool = options.iter().find_map(CastOption::slot_pool);
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
                <Icon name="wand" />
            </button>
            {(option_count > 1).then(move || {
                view! {
                    <Show when=move || picker_open.get()>
                        <div class="cast-slot-picker">
                            {options.with_value(|opts| {
                                opts.iter().map(|opt| {
                                    let highlight = opt.is_natural();
                                    let foreign = opt
                                        .slot_pool()
                                        .is_some_and(|pool| Some(pool) != native_slot_pool);
                                    let opt_clone = opt.clone();
                                    let opt_view = opt.clone().view(i18n, foreign);
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
                                <Icon name="x" />
                            </button>
                        </div>
                    </Show>
                }
            })}
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SpellSlotLevel;

    fn slots(arcane: &[(u32, u32, u32)], pact: &[(u32, u32, u32)]) -> SpellSlots {
        let mut spell_slots = SpellSlots::default();
        for (pool, levels) in [(SpellSlotPool::Arcane, arcane), (SpellSlotPool::Pact, pact)] {
            if levels.is_empty() {
                continue;
            }
            let pool_slots = spell_slots.entry(pool).or_default();
            for &(level, total, used) in levels {
                pool_slots[(level - 1) as usize] = SpellSlotLevel { total, used };
            }
        }
        spell_slots
    }

    #[test]
    fn slot_options_native_pool_first_then_foreign() {
        // Sorcerer spell L1; Arcane L1..L2 available, Pact L1 available.
        let slots = slots(&[(1, 2, 0), (2, 1, 0)], &[(1, 2, 1)]);
        let options: Vec<CastOption> =
            CastOption::slot_options(&slots, 1, SpellSlotPool::Arcane).collect();
        assert_eq!(options.len(), 3);
        assert!(matches!(
            options[0],
            CastOption::SpellSlot {
                pool: SpellSlotPool::Arcane,
                level: 1,
                remaining: 2,
                natural: true
            }
        ));
        assert!(matches!(
            options[1],
            CastOption::SpellSlot {
                pool: SpellSlotPool::Arcane,
                level: 2,
                natural: false,
                ..
            }
        ));
        assert!(matches!(
            options[2],
            CastOption::SpellSlot {
                pool: SpellSlotPool::Pact,
                level: 1,
                remaining: 1,
                natural: false
            }
        ));
    }

    #[test]
    fn slot_options_filters_level_and_exhausted() {
        // Spell L2: L1 slots never offered; exhausted L2 skipped, L3 offered.
        let slots = slots(&[(1, 4, 0), (2, 3, 3), (3, 2, 1)], &[]);
        let options: Vec<CastOption> =
            CastOption::slot_options(&slots, 2, SpellSlotPool::Arcane).collect();
        assert_eq!(options.len(), 1);
        assert!(matches!(
            options[0],
            CastOption::SpellSlot {
                level: 3,
                remaining: 1,
                natural: false,
                ..
            }
        ));
    }

    #[test]
    fn slot_options_single_pool_unchanged_and_empty() {
        // Absent second pool yields nothing extra; level above all slots → empty.
        let slots = slots(&[(1, 2, 0)], &[]);
        assert_eq!(
            CastOption::slot_options(&slots, 1, SpellSlotPool::Arcane).count(),
            1
        );
        assert_eq!(
            CastOption::slot_options(&slots, 5, SpellSlotPool::Arcane).count(),
            0
        );
    }

    #[test]
    fn slot_options_foreign_native_level_is_not_natural() {
        // Pact L1 paying a Sorcerer L1 spell: same level, foreign pool.
        let slots = slots(&[], &[(1, 2, 0)]);
        let options: Vec<CastOption> =
            CastOption::slot_options(&slots, 1, SpellSlotPool::Arcane).collect();
        assert!(matches!(
            options[0],
            CastOption::SpellSlot {
                pool: SpellSlotPool::Pact,
                natural: false,
                ..
            }
        ));
    }
}

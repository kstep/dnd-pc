use std::ops::Not;

use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr, tr};
use reactive_stores::Store;

use crate::model::{Character, CharacterStoreFields, Translatable};

#[component]
pub fn SpellSlotsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let spell_slots = store.spell_slots();

    let pools = move || {
        let pools = spell_slots
            .read()
            .iter()
            .filter(|(_, slots)| slots.iter().any(|s| s.total > 0))
            .map(|(&pool, _)| pool)
            .collect::<Vec<_>>();
        pools.is_empty().not().then_some(pools)
    };

    let i18n = expect_context::<I18n>();
    Some(view! {
        <h4 class="summary-subsection-title">{move_tr!("spell-slots")}</h4>
        {move || Some(pools()?.into_iter().map(|pool| {
            let guard = spell_slots.read();
            let slots = (1..=9u32)
                .filter_map(|level| {
                    let idx = (level - 1) as usize;
                    let slot = guard.get(&pool)
                        .and_then(|s| s.get(idx))
                        .copied()
                        .unwrap_or_default();
                    (slot.total > 0).then_some((level, idx, slot))
                })
                .collect::<Vec<_>>();
            view! {
                <h5 class="pool-header">{i18n.tr(pool.tr_key())}</h5>
                <div class="summary-spell-slots">
                    {slots.into_iter().map(|(level, idx, slot)| {
                        let remaining = move || slot.total.saturating_sub(slot.used);
                        view! {
                            <div class="summary-slot">
                                <span class="summary-slot-level">
                                    {tr!("slot-level", {"level" => level.to_string()})}
                                </span>
                                <input
                                    type="number"
                                    class="short-input"
                                    min="0"
                                    prop:max=slot.total.to_string()
                                    prop:value=slot.used.to_string()
                                    on:input=move |e| {
                                        if let Ok(value) = event_target_value(&e).parse() {
                                            spell_slots.update(|pools| {
                                                if let Some(slots) = pools.get_mut(&pool) {
                                                    slots[idx].used = value;
                                                }
                                            });
                                        }
                                    }
                                />
                                <span>"/" {slot.total} " (" {remaining} ")"</span>
                            </div>
                        }
                    }).collect_view()}
                </div>
            }
        }).collect_view())}
    })
}

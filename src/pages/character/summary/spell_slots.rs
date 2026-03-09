use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::resource_slot::ResourceSlot,
    model::{Character, CharacterStoreFields, Translatable},
};

#[component]
pub fn SpellSlotsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let spell_slots = store.spell_slots();

    let i18n = expect_context::<I18n>();

    move || {
        let pools = spell_slots
            .read()
            .iter()
            .filter(|(_, slots)| slots.iter().any(|s| s.total > 0))
            .map(|(&pool, _)| pool)
            .collect::<Vec<_>>();

        if pools.is_empty() {
            return None;
        }

        let many_pools = pools.len() > 1;
        let view = pools.into_iter().map(|pool| {
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
                {many_pools.then(|| view! { <h5 class="pool-header">{i18n.tr(pool.tr_key())}</h5> })}
                <div class="summary-spell-slots">
                    {slots.into_iter().map(|(level, idx, slot)| {
                        let label = tr!("slot-level", {"level" => level.to_string()});
                        view! {
                            <ResourceSlot
                                label=label
                                max=slot.total
                                used=slot.used
                                on_change=move |value| {
                                    spell_slots.update(|pools| {
                                        if let Some(slots) = pools.get_mut(&pool) {
                                            slots[idx].used = value;
                                        }
                                    });
                                }
                            />
                        }
                    }).collect_view()}
                </div>
            }
        }).collect_view();

        if view.is_empty() {
            return None;
        }

        Some(view! {
            <h4 class="summary-subsection-title">{move_tr!("spell-slots")}</h4>
            {view}
        })
    }
}

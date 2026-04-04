use leptos::{either::Either, prelude::*};
use leptos_fluent::{move_tr, tr};
use reactive_stores::Store;

use crate::{
    components::resource_slot::ResourceSlot,
    model::{
        Character, CharacterIdentityStoreFields, CharacterStoreFields, FeatureValue, Translatable,
    },
};

#[component]
pub fn ResourcesBlock() -> impl IntoView {
    let i18n = expect_context::<leptos_fluent::I18n>();
    let store = expect_context::<Store<Character>>();
    let feature_data = store.feature_data();
    let spell_slots = store.spell_slots();
    let classes = store.identity().classes();

    let spell_slots = move || {
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
                <div class="session-spell-slots">
                    {slots.into_iter().map(|(level, idx, slot)| {
                        let label = tr!("slot-level", {"level" => level});
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
            <h4 class="session-subsection-title">{move_tr!("spell-slots")}</h4>
            {view}
        })
    };

    let hit_dice = move || {
        let dice = classes
            .read()
            .iter()
            .enumerate()
            .filter(|(_, cl)| cl.level > 0)
            .map(|(i, cl)| {
                let label = format!("{} d{}", cl.class_label(), cl.hit_die_sides);
                let max = cl.level;
                let used = cl.hit_dice_used;
                view! {
                    <ResourceSlot
                        label=label
                        max=max
                        used=used
                        on_change=move |value| {
                            classes.write()[i].hit_dice_used = value;
                        }
                    />
                }
            })
            .collect::<Vec<_>>();

        if dice.is_empty() {
            None
        } else {
            Some(view! {
                <h4 class="session-subsection-title">{move_tr!("hit-dice")}</h4>
                <div class="session-spell-slots">
                    {dice}
                </div>
            })
        }
    };

    let resources = move || {
        let res = feature_data
            .read()
            .iter()
            .flat_map(|(feat_name, entry)| {
                entry
                    .fields
                    .iter()
                    .enumerate()
                    .filter_map(|(field_idx, field)| match &field.value {
                        FeatureValue::Points { used, max } if *max > 0 => {
                            let used = *used;
                            let max = *max;
                            let label = field.label().to_string();
                            let feat_name = feat_name.clone();

                            Some(Either::Left(view! {
                                <ResourceSlot
                                    label=label
                                    max=max
                                    used=used
                                    on_change=move |value| {
                                        feature_data.update(|map| {
                                            if let Some(entry) = map.get_mut(&feat_name)
                                                && let Some(field) = entry.fields.get_mut(field_idx)
                                                && let FeatureValue::Points { used, .. } = &mut field.value
                                            {
                                                *used = value;
                                            }
                                        });
                                    }
                                />
                            }))
                        }
                        FeatureValue::Die { die, used } if die.amount > 0 => {
                            let used = *used;
                            let max = die.amount;
                            let label = format!("{} ({})", field.label(), die);
                            let feat_name = feat_name.clone();

                            Some(Either::Right(view! {
                                <ResourceSlot
                                    label=label
                                    max=max
                                    used=used
                                    on_change=move |value| {
                                        feature_data.update(|map| {
                                            if let Some(entry) = map.get_mut(&feat_name)
                                                && let Some(field) = entry.fields.get_mut(field_idx)
                                                && let FeatureValue::Die { used, .. } = &mut field.value
                                            {
                                                *used = value;
                                            }
                                        });
                                    }
                                />
                            }))
                        }
                        _ => None,
                    })
            })
            .collect::<Vec<_>>();

        if res.is_empty() {
            None
        } else {
            Some(view! {
                <h4 class="session-subsection-title">{move_tr!("session-resources")}</h4>
                <div class="session-spell-slots">
                    {res}
                </div>
            })
        }
    };

    view! {
        <div class="session-section session-section-resources" id="session-resources">
            <h3 class="session-section-title">{move_tr!("session-resources")}</h3>
            {spell_slots}
            {hit_dice}
            {resources}
        </div>
    }
}

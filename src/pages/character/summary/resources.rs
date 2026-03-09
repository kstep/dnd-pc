use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::resource_slot::ResourceSlot,
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields, FeatureValue},
};

#[component]
pub fn ResourcesBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let feature_data = store.feature_data();
    let classes = store.identity().classes();

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
                <h4 class="summary-subsection-title">{move_tr!("hit-dice")}</h4>
                <div class="summary-spell-slots">
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
                <h4 class="summary-subsection-title">{move_tr!("summary-resources")}</h4>
                <div class="summary-spell-slots">
                    {res}
                </div>
            })
        }
    };

    move || {
        let hit_dice = hit_dice();
        let resources = resources();

        if hit_dice.is_none() && resources.is_none() {
            None
        } else {
            Some(view! {
                {hit_dice}
                {resources}
            })
        }
    }
}

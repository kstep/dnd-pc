use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::model::{Character, CharacterStoreFields, FeatureValue};

#[component]
pub fn ResourcesBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let feature_data = store.feature_data();

    let resources = move || {
        feature_data
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
                                <div class="summary-slot">
                                    <span class="summary-slot-level">{label}</span>
                                    <input
                                        type="number"
                                        class="short-input"
                                        min="0"
                                        prop:max=max.to_string()
                                        prop:value=used.to_string()
                                        on:input={
                                            move |event| {
                                                if let Ok(value) = event_target_value(&event).parse() {
                                                    feature_data.update(|map| {
                                                        if let Some(entry) = map.get_mut(&feat_name)
                                                            && let Some(field) = entry.fields.get_mut(field_idx)
                                                            && let FeatureValue::Points { used, .. } = &mut field.value
                                                        {
                                                            *used = value;
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    />
                                    <span>"/" {max}</span>
                                </div>
                            }))
                        },
                        FeatureValue::Die(value) if !value.is_empty() => Some(
                            Either::Right(view! {
                                <div class="summary-slot">
                                    <span class="summary-slot-level">{field.label().to_string()}</span>
                                    <span>{value.clone()}</span>
                                </div>
                            })
                        ),
                        _ => None,
                    })
            })
            .collect::<Vec<_>>()
    };

    move || {
        let resources = resources();

        if resources.is_empty() {
            None
        } else {
            Some(view! {
                <h4 class="summary-subsection-title">{move_tr!("summary-resources")}</h4>
                <div class="summary-spell-slots">
                    {resources}
                </div>
            })
        }
    }
}

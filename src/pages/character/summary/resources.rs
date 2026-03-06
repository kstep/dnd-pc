use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::model::{Character, CharacterStoreFields, FeatureValue};

#[component]
pub fn ResourcesBlock() -> impl IntoView {
    enum ResourceKind {
        Points { used: u32, max: u32 },
        Die(String),
    }

    let store = expect_context::<Store<Character>>();
    let feature_data = store.feature_data();

    let resources = feature_data
        .read()
        .iter()
        .flat_map(|(feat_name, entry)| {
            entry
                .fields
                .iter()
                .enumerate()
                .filter_map(|(field_idx, field)| match &field.value {
                    FeatureValue::Points { used, max } if *max > 0 => Some((
                        feat_name.clone(),
                        field_idx,
                        field.label().to_string(),
                        ResourceKind::Points {
                            used: *used,
                            max: *max,
                        },
                    )),
                    FeatureValue::Die(val) if !val.is_empty() => Some((
                        feat_name.clone(),
                        field_idx,
                        field.label().to_string(),
                        ResourceKind::Die(val.clone()),
                    )),
                    _ => None,
                })
        })
        .collect::<Vec<_>>();

    if resources.is_empty() {
        return None;
    }

    Some(view! {
        <h4 class="summary-subsection-title">{move_tr!("summary-resources")}</h4>
        <div class="summary-spell-slots">
            {resources.into_iter().map(|(feat_name, field_idx, label, kind)| {
                match kind {
                    ResourceKind::Points { used, max } => {
                        Either::Left(view! {
                            <div class="summary-slot">
                                <span class="summary-slot-level">{label}</span>
                                <input
                                    type="number"
                                    class="short-input"
                                    min="0"
                                    prop:max=max.to_string()
                                    prop:value=used.to_string()
                                    on:input={
                                        let feat_name = feat_name.clone();
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
                        })
                    }
                    ResourceKind::Die(val) => {
                        Either::Right(view! {
                            <div class="summary-slot">
                                <span class="summary-slot-level">{label}</span>
                                <span>{val}</span>
                            </div>
                        })
                    }
                }
            }).collect_view()}
        </div>
    })
}

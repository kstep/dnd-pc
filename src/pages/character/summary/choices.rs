use std::collections::HashMap;

use leptos::{either::Either, prelude::*};
use reactive_stores::Store;

use crate::{
    components::summary_list::{SummaryList, SummaryListItem},
    model::{Character, CharacterStoreFields, FeatureOption, FeatureValue},
    rules::{ChoiceOptions, FieldKind, RulesRegistry},
};

#[component]
pub fn ChoicesBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();

    let identity = store.identity();
    let feature_data = store.feature_data();

    move || {
        let features = feature_data.read();
        let remaining_points = features
            .values()
            .flat_map(|field| {
                field.fields.iter().filter_map(|field| {
                    Some((field.name.as_str(), field.value.available_points()?))
                })
            })
            .collect::<HashMap<_, _>>();

        features
            .iter()
            .filter_map(|(feat_name, entry)| {
                let id = identity.read();
                let fields = registry.with_feature(&id, feat_name, |feat| {
                    feat.fields
                        .iter()
                        .filter_map(|(name, def)| {
                            let FieldKind::Choice { options, cost, .. } = &def.kind else {
                                return None;
                            };

                            let from = if let ChoiceOptions::Ref { from } = options {
                                Some(from.clone())
                            } else {
                                None
                            };

                            let points = cost
                                .as_deref()
                                .and_then(|cost| remaining_points.get(cost))
                                .copied()
                                .unwrap_or_default();

                            Some((name.clone(), (points, from, cost.clone())))
                        })
                        .collect::<HashMap<_, _>>()
                })?;

                Some(entry.fields.iter().enumerate().filter_map(move |(field_index, field)| {
                    let (points, from, cost) = fields.get(&field.name)?;
                    let short = cost.as_deref().and_then(|c| registry.get_points_short(&id, c));

                    let FeatureValue::Choice { options } = &field.value else {
                        return None;
                    };

                    Some(match from {
                        None => Either::Left({
                            let selected = options
                                .iter()
                                .filter(|opt| opt.cost <= *points)
                                .map(|opt| SummaryListItem {
                                    name: opt.label().to_string(),
                                    description: opt.description.clone(),
                                    badge: (opt.cost > 0).then(|| {
                                        view! {
                                            <span class="summary-choice-cost">{opt.cost}</span>
                                        }
                                        .into_any()
                                    }),
                                })
                                .collect::<Vec<_>>();
                            if selected.is_empty() {
                                return None;
                            }

                            let style = short.as_ref().map(|s| format!("--points-symbol: '{s}'"));
                            Some(view! {
                                <div class="summary-subsection" style=style>
                                    <h4 class="summary-subsection-title">{field.label().to_string()}</h4>
                                    <SummaryList items=selected />
                                </div>
                            })
                        }),
                        Some(from) => Either::Right({
                            let from_field = entry.fields.iter().find(|field| &field.name == from)?;
                            let FeatureValue::Choice { options: from_options } = &from_field.value else {
                                return None;
                            };
                            let from_options = from_options.clone();
                            let feat_name = feat_name.to_string();

                            let choice_entry_factory = move |(index, current): (usize, &FeatureOption)| {
                                let current = current.clone();
                                let feat_name = feat_name.clone();
                                let from_options = from_options.clone();
                                view! {
                                    <div class="choice-entry">
                                        <select on:change={move |event| {
                                            let value = event_target_value(&event);
                                            let Some(selected_option) = from_options.iter().find(|opt| opt.name == value) else {
                                                return;
                                            };

                                            feature_data.update(|features| {
                                                if let Some(entry) = features.get_mut(&feat_name)
                                                    && let Some(field) = entry.fields.get_mut(field_index)
                                                    && let FeatureValue::Choice { options } = &mut field.value
                                                    && let Some(option) = options.get_mut(index)
                                                {
                                                    option.clone_from(selected_option);
                                                }
                                            });
                                        }}>
                                            <option value="">""</option>
                                            {from_options.iter().map(|opt| {
                                                view! {
                                                    <option value=opt.name.clone() selected={opt.name == current.name}>{opt.label().to_string()}</option>
                                                }
                                            }).collect_view()}
                                        </select>
                                    </div>
                                }
                            };

                            view! {
                                <div class="summary-subsection">
                                    <h4 class="summary-subsection-title">{field.label().to_string()}</h4>
                                    <div class="choice-list">
                                        {options.iter().enumerate().map(choice_entry_factory).collect_view()}
                                    </div>
                                </div>
                            }
                        })
                    })
                }))
            })
            .flatten()
            .collect_view()
    }
}

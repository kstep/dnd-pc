use std::collections::BTreeMap;

use leptos::{either::Either, prelude::*};
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::{CastButton, CastOption},
        summary_list::{SummaryList, SummaryListItem},
    },
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
            .collect::<BTreeMap<_, _>>();

        let id = identity.read();
        features
            .iter()
            .filter_map(|(feat_name, entry)| {
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

                            Some((name.to_string(), (points, from, cost.clone())))
                        })
                        .collect::<BTreeMap<_, _>>()
                })?;

                let feat_name = feat_name.clone();
                let id = &id;
                Some(entry.fields.iter().enumerate().filter_map(move |(field_index, field)| {
                    let (points, from, cost) = fields.get(&field.name)?;
                    let short = cost.as_deref().and_then(|c| registry.get_points_short(id, c));
                    let cost_field = cost.as_ref().map(|c| StoredValue::new(c.clone()));
                    let feat_name = feat_name.clone();

                    let FeatureValue::Choice { options } = &field.value else {
                        return None;
                    };

                    Some(match from {
                        None => Either::Left({
                            let selected = options
                                .iter()
                                .filter(|opt| opt.cost <= *points)
                                .map(|opt| {
                                    let opt_cost = opt.cost;
                                    let can_cast = cost_field.is_some() && opt_cost > 0 && opt_cost <= *points;
                                    SummaryListItem {
                                        name: opt.label().to_string(),
                                        description: opt.description.clone(),
                                        badge: (opt.cost > 0).then(|| {
                                            view! {
                                                <span class="summary-choice-cost">{opt.cost}</span>
                                                {cost_field.map(|cfn| view! {
                                                    <CastButton
                                                        disabled=!can_cast
                                                        on_cast={Callback::new(move |_: CastOption| {
                                                            cfn.with_value(|cost_name| {
                                                                feature_data.update(|map| {
                                                                    for entry in map.values_mut() {
                                                                        if let Some(field) = entry.fields.iter_mut().find(|f| f.name == *cost_name)
                                                                            && let FeatureValue::Points { used, max } = &mut field.value
                                                                        {
                                                                            *used = (*used + opt_cost).min(*max);
                                                                            break;
                                                                        }
                                                                    }
                                                                });
                                                            });
                                                        })}
                                                    />
                                                })}
                                            }
                                            .into_any()
                                        }),
                                    }
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
                            let from_options = StoredValue::new(from_options.clone());
                            let feat_name = StoredValue::new(feat_name.to_string());

                            let choice_entry_factory = move |(index, current): (usize, &FeatureOption)| {
                                let current_name = current.name.clone();
                                view! {
                                    <div class="choice-entry">
                                        <select on:change={move |event| {
                                            let value = event_target_value(&event);
                                            from_options.with_value(|opts| {
                                                let Some(selected_option) = opts.iter().find(|opt| opt.name == value) else {
                                                    return;
                                                };
                                                feat_name.with_value(|name| {
                                                    feature_data.update(|features| {
                                                        if let Some(entry) = features.get_mut(name)
                                                            && let Some(field) = entry.fields.get_mut(field_index)
                                                            && let FeatureValue::Choice { options } = &mut field.value
                                                            && let Some(option) = options.get_mut(index)
                                                        {
                                                            option.clone_from(selected_option);
                                                        }
                                                    });
                                                });
                                            });
                                        }}>
                                            <option value="">""</option>
                                            {from_options.with_value(|opts| opts.iter().map(|opt| {
                                                view! {
                                                    <option value=opt.name.clone() selected={opt.name == current_name}>{opt.label().to_string()}</option>
                                                }
                                            }).collect_view())}
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

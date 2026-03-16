use std::collections::BTreeMap;

use leptos::{either::EitherOf3, prelude::*};
use leptos_fluent::I18n;
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::{CastButton, CastOption},
        icon::Icon,
        summary_list::{SummaryList, SummaryListItem},
    },
    model::{Character, CharacterStoreFields, FeatureOption, FeatureValue, Translatable},
    rules::{ChoiceOption, ChoiceOptions, FieldKind, RulesRegistry},
};

/// Info extracted from the registry for a single Choice field.
struct ChoiceFieldInfo {
    points: u32,
    from: Option<String>,
    cost: Option<String>,
    /// Definition options that have `action` set (action menu items).
    action_options: Vec<ChoiceOption>,
}

#[component]
pub fn ChoicesBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();
    let i18n = expect_context::<I18n>();

    let identity = store.identity();
    let feature_data = store.feature_data();

    move || {
        let features = feature_data.read();
        let remaining_points = features
            .values()
            .flat_map(|entry| {
                entry.fields.iter().filter_map(|field| {
                    Some((field.name.as_str(), field.value.available_points()?))
                })
            })
            .collect::<BTreeMap<_, _>>();

        let char_level = store.read().level();
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

                            // Collect action options from definition
                            let action_options = match options {
                                ChoiceOptions::List(list) => list
                                    .iter()
                                    .filter(|o| o.action.is_some() && o.level <= char_level)
                                    .cloned()
                                    .collect(),
                                _ => Vec::new(),
                            };

                            Some((name.to_string(), ChoiceFieldInfo {
                                points,
                                from,
                                cost: cost.clone(),
                                action_options,
                            }))
                        })
                        .collect::<BTreeMap<_, _>>()
                })?;

                let feat_name = feat_name.clone();
                let id = &id;
                Some(entry.fields.iter().enumerate().filter_map(move |(field_index, field)| {
                    let info = fields.get(&field.name)?;
                    let short = info.cost.as_deref().and_then(|c| registry.get_points_short(id, c));
                    let cost_field = info.cost.as_ref().map(|c| StoredValue::new(c.clone()));
                    let feat_name = feat_name.clone();
                    let points = info.points;

                    let FeatureValue::Choice { options } = &field.value else {
                        return None;
                    };

                    Some(match &info.from {
                        // Action menu: definition options with action types, no stored options
                        None if options.is_empty() && !info.action_options.is_empty() => EitherOf3::A({
                            let items = info.action_options
                                .iter()
                                .filter(|opt| opt.cost <= points)
                                .map(|opt| {
                                    let opt_cost = opt.cost;
                                    let action_icon = opt.action.map(|a| {
                                        let title = i18n.tr(a.tr_key());
                                        view! {
                                            <Icon name=a.icon_name() size=14 title=title />
                                        }
                                    });

                                    let cost_badge = (opt.cost > 0).then(|| {
                                        view! {
                                            <span class="summary-choice-cost">{opt.cost}</span>
                                            {cost_field.map(|cfn| view! {
                                                <CastButton
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
                                    });

                                    SummaryListItem {
                                        name: opt.label().to_string(),
                                        description: opt.description.clone(),
                                        badge: Some(view! {
                                            {action_icon}
                                            {cost_badge}
                                        }.into_any()),
                                    }
                                })
                                .collect::<Vec<_>>();

                            if items.is_empty() {
                                return None;
                            }
                            let style = short.as_ref().map(|s| format!("--points-symbol: '{s}'"));
                            Some(view! {
                                <div class="summary-subsection" style=style>
                                    <h4 class="summary-subsection-title">{field.label().to_string()}</h4>
                                    <SummaryList items=items />
                                </div>
                            })
                        }),
                        // Regular stored choices (non-ref)
                        None => EitherOf3::B({
                            let selected = options
                                .iter()
                                .filter(|opt| opt.cost <= points)
                                .map(|opt| {
                                    let opt_cost = opt.cost;
                                    SummaryListItem {
                                        name: opt.label().to_string(),
                                        description: opt.description.clone(),
                                        badge: (opt.cost > 0).then(|| {
                                            view! {
                                                <span class="summary-choice-cost">{opt.cost}</span>
                                                {cost_field.map(|cfn| view! {
                                                    <CastButton
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
                        // Ref-based choices (dropdown selects)
                        Some(from) => EitherOf3::C({
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

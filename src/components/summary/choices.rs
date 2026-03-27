use std::collections::BTreeMap;

use leptos::{prelude::*, tachys::view::any_view::AnyView};
use leptos_fluent::I18n;
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::{CastButton, CastOption},
        effects_calc_modal::{
            EffectsCalcInfo, EffectsCalcModal, all_self_effects_diceless, apply_self_effects_now,
            inject_resource_vars,
        },
        icon::Icon,
        summary_list::{SummaryList, SummaryListItem},
    },
    model::{
        Attribute, Character, CharacterStoreFields, EffectDefinition, EffectRange, FeatureOption,
        FeatureValue, Translatable, short_name,
    },
    rules::{ActionType, ChoiceOption, ChoiceOptions, FieldKind, RulesRegistry},
};

/// Info extracted from the registry for a single Choice field.
struct ChoiceFieldInfo {
    points: u32,
    from: Option<String>,
    cost: Option<String>,
    /// Definition options that have `action` set (action menu items).
    action_options: Vec<ChoiceOption>,
}

/// Input for a single choice/action item passed to `build_choice_items`.
struct ChoiceItemInput {
    name: String,
    description: String,
    cost: u32,
    action: Option<ActionType>,
    effects: Vec<EffectDefinition>,
    feature_name: String,
}

/// Build SummaryListItems from an iterator of choice/action items.
fn build_choice_items(
    items: impl Iterator<Item = ChoiceItemInput>,
    points: u32,
    spend_cost: Option<Callback<u32>>,
    open_effects: Callback<(String, String, Vec<EffectDefinition>)>,
    i18n: &I18n,
) -> Vec<SummaryListItem> {
    items
        .filter(|item| item.cost <= points)
        .map(|item| {
            let action_icon = item.action.map(|action_type| {
                let title = untrack(|| i18n.tr(action_type.tr_key()).into_owned());
                view! {
                    <Icon name=action_type.icon_name() size=14 title=title />
                }
            });

            let has_effects = !item.effects.is_empty();
            let show_button = item.cost > 0 || has_effects;

            let cost_badge = (item.cost > 0).then(|| {
                view! {
                    <span class="summary-choice-cost">{item.cost}</span>
                }
            });

            let cast_button = show_button.then(|| {
                let feature_name = item.feature_name.clone();
                let option_label = item.name.clone();
                let effects = item.effects;
                let cost = item.cost;
                let on_cast = Callback::new(move |_: CastOption| {
                    if cost > 0
                        && let Some(spend) = spend_cost
                    {
                        spend.run(cost);
                    }
                    if has_effects {
                        open_effects.run((
                            feature_name.clone(),
                            option_label.clone(),
                            effects.clone(),
                        ));
                    }
                });
                view! { <CastButton on_cast /> }
            });

            SummaryListItem {
                name: item.name,
                description: item.description,
                badge: if action_icon.is_some() || cost_badge.is_some() || cast_button.is_some() {
                    Some(
                        view! {
                            {action_icon}
                            {cost_badge}
                            {cast_button}
                        }
                        .into_any(),
                    )
                } else {
                    None
                },
            }
        })
        .collect()
}

/// A group of choice items to be rendered under a shared header.
struct ChoiceGroup {
    short: Option<String>,
    items: Vec<SummaryListItem>,
}

#[component]
pub fn ChoicesBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<crate::effective::EffectiveCharacter>();
    let i18n = expect_context::<I18n>();

    let feature_data = store.feature_data();

    // Effects calculator modal state
    let show_calc = RwSignal::new(false);
    let calc_info = StoredValue::new(None::<EffectsCalcInfo>);

    let open_effects = Callback::new(
        move |(feature_name, option_label, effects): (String, String, Vec<EffectDefinition>)| {
            let character = store.read_untracked();
            let class_level = registry
                .feature_class_level(&character.identity, &feature_name)
                .unwrap_or(character.level());
            let mut extra_vars = BTreeMap::new();
            extra_vars.insert(Attribute::ClassLevel, class_level as i32);

            // Inject Points field values if feature has one
            if let Some(entry) = character.feature_data.get(&feature_name) {
                inject_resource_vars(&mut extra_vars, entry);
            }

            // All effects are Caster with no dice — apply immediately, skip modal
            let all_caster = effects.iter().all(|e| e.range == EffectRange::Caster);
            if all_caster && all_self_effects_diceless(&effects, &character, &extra_vars) {
                drop(character);
                apply_self_effects_now(
                    &effects,
                    &option_label,
                    &feature_name,
                    &extra_vars,
                    &store,
                    eff.effects(),
                );
                return;
            }

            calc_info.set_value(Some(EffectsCalcInfo {
                title: option_label.clone(),
                effects,
                extra_vars,
                spell_name: option_label,
                feature_name: feature_name.clone(),
            }));
            show_calc.set(true);
        },
    );

    let choices_view = move || {
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

        // Collect grouped choice items and standalone ref-based views
        let mut groups: BTreeMap<String, ChoiceGroup> = BTreeMap::new();
        let mut ref_views: Vec<AnyView> = Vec::new();

        for (feat_name, entry) in features.iter() {
            let Some(fields) = registry.with_feature(feat_name, |feat| {
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

                        let action_options = match options {
                            ChoiceOptions::List(list) => list
                                .iter()
                                .filter(|o| o.action.is_some() && o.level <= char_level)
                                .cloned()
                                .collect(),
                            _ => Vec::new(),
                        };

                        Some((
                            name.to_string(),
                            ChoiceFieldInfo {
                                points,
                                from,
                                cost: cost.clone(),
                                action_options,
                            },
                        ))
                    })
                    .collect::<BTreeMap<_, _>>()
            }) else {
                continue;
            };

            for (field_index, field) in entry.fields.iter().enumerate() {
                let Some(info) = fields.get(&field.name) else {
                    continue;
                };
                let short = info.cost.as_deref().map(short_name);
                let points = info.points;

                let spend_cost =
                    info.cost
                        .as_ref()
                        .map(|c| StoredValue::new(c.clone()))
                        .map(|cfn| {
                            Callback::new(move |opt_cost: u32| {
                                cfn.with_value(|cost_name| {
                                    feature_data.update(|map| {
                                        for entry in map.values_mut() {
                                            if let Some(field) = entry
                                                .fields
                                                .iter_mut()
                                                .find(|f| f.name == *cost_name)
                                            {
                                                match &mut field.value {
                                                    FeatureValue::Points { used, max } => {
                                                        *used = (*used + opt_cost).min(*max);
                                                    }
                                                    FeatureValue::Die { die, used } => {
                                                        *used = (*used + opt_cost).min(die.amount);
                                                    }
                                                    _ => continue,
                                                }
                                                break;
                                            }
                                        }
                                    });
                                });
                            })
                        });

                let FeatureValue::Choice { options } = &field.value else {
                    continue;
                };

                let label = field.label().to_string();

                match &info.from {
                    // Action menu or regular stored choices — group by label
                    None if options.is_empty() && !info.action_options.is_empty() => {
                        let items = build_choice_items(
                            info.action_options.iter().map(|opt| ChoiceItemInput {
                                name: opt.label().to_string(),
                                description: opt.description.clone(),
                                cost: opt.cost,
                                action: opt.action,
                                effects: opt.effects.clone(),
                                feature_name: feat_name.clone(),
                            }),
                            points,
                            spend_cost,
                            open_effects,
                            &i18n,
                        );
                        let group = groups.entry(label).or_insert_with(|| ChoiceGroup {
                            short: short.clone(),
                            items: Vec::new(),
                        });
                        group.items.extend(items);
                    }
                    None => {
                        let items = build_choice_items(
                            options.iter().map(|opt| ChoiceItemInput {
                                name: opt.label().to_string(),
                                description: opt.description.clone(),
                                cost: opt.cost,
                                action: None,
                                effects: Vec::new(),
                                feature_name: feat_name.clone(),
                            }),
                            points,
                            spend_cost,
                            open_effects,
                            &i18n,
                        );
                        let group = groups.entry(label).or_insert_with(|| ChoiceGroup {
                            short: short.clone(),
                            items: Vec::new(),
                        });
                        group.items.extend(items);
                    }
                    // Ref-based choices (dropdown selects) — render standalone
                    Some(from) => {
                        let Some(from_field) =
                            entry.fields.iter().find(|field| &field.name == from)
                        else {
                            continue;
                        };
                        let FeatureValue::Choice {
                            options: from_options,
                        } = &from_field.value
                        else {
                            continue;
                        };
                        let from_options = StoredValue::new(from_options.clone());
                        let feat_name = StoredValue::new(feat_name.clone());

                        let choice_entry_factory = move |(index, current): (
                            usize,
                            &FeatureOption,
                        )| {
                            let current_name = current.name.clone();
                            view! {
                                <div class="entry-item">
                                    <div class="entry-content">
                                        <select class="entry-name" on:change={move |event| {
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
                                    <div class="entry-actions" />
                                </div>
                            }
                        };

                        ref_views.push(
                            view! {
                                <div class="summary-subsection">
                                    <h4 class="summary-subsection-title">{label}</h4>
                                    <div class="entry-list">
                                        {options.iter().enumerate().map(choice_entry_factory).collect_view()}
                                    </div>
                                </div>
                            }
                            .into_any(),
                        );
                    }
                }
            }
        }

        // Render grouped sections
        let grouped_views: Vec<AnyView> = groups
            .into_iter()
            .filter(|(_, group)| !group.items.is_empty())
            .map(|(label, group)| {
                let style = group.short.map(|s| format!("--points-symbol: '{s}'"));
                view! {
                    <div class="summary-subsection" style=style>
                        <h4 class="summary-subsection-title">{label}</h4>
                        <SummaryList items=group.items />
                    </div>
                }
                .into_any()
            })
            .collect();

        view! {
            {grouped_views}
            {ref_views}
        }
    };

    view! {
        {choices_view}
        <EffectsCalcModal show=show_calc info=calc_info />
    }
}

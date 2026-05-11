use std::collections::BTreeMap;

use leptos::{prelude::*, tachys::view::any_view::AnyView};
use leptos_fluent::I18n;
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::{CastButton, CastOption},
        effects_calc_modal::{
            EffectsCalcInfo, EffectsCalcModal, all_self_effects_diceless, apply_self_effects_now,
            inject_resource_vars, open_calc_modal,
        },
        icon::Icon,
        session_list::{SessionList, SessionListItem},
    },
    model::{
        ActionType, AttrKey, Attribute, Character, CharacterCoreStoreFields, CharacterStoreFields,
        EffectDefinition, FeatureOption, FeatureValue, FeaturesStoreFields, Translatable,
        short_name,
    },
    rules::{ChoiceOption, ChoiceOptions, RulesRegistry},
};

/// Info extracted from the registry for a single action.
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

/// Build SessionListItems from an iterator of choice/action items.
fn build_choice_items(
    items: impl Iterator<Item = ChoiceItemInput>,
    points: u32,
    spend_cost: Option<Callback<u32>>,
    open_effects: Callback<(String, String, Vec<EffectDefinition>)>,
    i18n: &I18n,
) -> Vec<SessionListItem> {
    items
        .filter(|item| item.cost <= points)
        .map(|item| {
            let action_icon = item.action.map(|action_type| {
                let title = untrack(|| i18n.tr(action_type.tr_key()).into_owned());
                view! {
                    <span class="entry-badge" title=title>
                        <Icon name=action_type.icon_name() />
                    </span>
                }
            });

            let has_effects = !item.effects.is_empty();
            let show_button = item.cost > 0 || has_effects;

            let cost_badge = (item.cost > 0).then(|| {
                view! {
                    <span class="entry-badge session-choice-cost">{item.cost}</span>
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
                view! { <CastButton on_cast /> }.into_any()
            });

            SessionListItem {
                name: item.name,
                description: item.description,
                badge: if action_icon.is_some() || cost_badge.is_some() {
                    Some(
                        view! {
                            <span class="entry-badge">
                                {action_icon}
                                {cost_badge}
                            </span>
                        }
                        .into_any(),
                    )
                } else {
                    None
                },
                actions: cast_button,
                name_prefix: None,
                name_extra: None,
                description_view: None,
            }
        })
        .collect()
}

/// A group of choice items to be rendered under a shared header.
struct ChoiceGroup {
    short: Option<String>,
    items: Vec<SessionListItem>,
}

#[component]
pub fn ChoicesBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<crate::effective::EffectiveCharacter>();
    let i18n = expect_context::<I18n>();

    let feature_data = store.core().features().data();

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
            extra_vars.insert(Attribute::ClassLevel(AttrKey::Scoped), class_level as i32);

            // Inject Points field values if feature has one
            if let Some(entry) = character.features.get(feature_name.as_str()) {
                inject_resource_vars(&mut extra_vars, entry);
            }

            // All effects are Caster with no dice — apply immediately, skip modal
            let all_caster = effects.iter().all(|e| e.range.can_target_self());
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

            open_calc_modal(
                show_calc,
                calc_info,
                EffectsCalcInfo {
                    title: option_label.clone(),
                    effects,
                    extra_vars,
                    spell_name: option_label,
                    feature_name: feature_name.clone(),
                },
            );
        },
    );

    let choices_view = move || {
        let features = feature_data.read();
        let remaining_points = features
            .values()
            .flat_map(|entry| {
                entry.fields.iter().filter_map(|field| {
                    let available = field.value.available_points()?;
                    let max = field.value.max_points()?;
                    Some((field.name.as_str(), (available, max)))
                })
            })
            .collect::<BTreeMap<_, _>>();

        let char_level = store.read().level();

        // Collect grouped choice items and standalone ref-based views
        let mut groups: BTreeMap<String, ChoiceGroup> = BTreeMap::new();
        let mut ref_views: Vec<AnyView> = Vec::new();

        for (feat_name, entry) in features.iter() {
            // Scoped fallback: when an action has no explicit cost reference,
            // spend from this feature's single Points/Die pool (lazy-created
            // by assign expressions). Matches the pure-assign convention
            // where pool name == feature name without redundant data wiring.
            let scoped_pool = entry
                .fields
                .iter()
                .find(|field| field.value.available_points().is_some())
                .map(|field| field.name.clone());

            let Some(actions) = registry.with_feature(feat_name, |feat| {
                feat.actions
                    .iter()
                    .map(|(name, action_def)| {
                        let from = if let ChoiceOptions::Ref { from } = &action_def.options {
                            Some(from.clone())
                        } else {
                            None
                        };

                        let cost = action_def.cost.clone().or_else(|| scoped_pool.clone());

                        let (points, _max_points) = cost
                            .as_deref()
                            .and_then(|cost| remaining_points.get(cost))
                            .copied()
                            .unwrap_or_default();

                        let action_options = match &action_def.options {
                            ChoiceOptions::List(list) => list
                                .iter()
                                .filter(|opt| opt.action.is_some() && opt.level <= char_level)
                                .cloned()
                                .collect(),
                            _ => Vec::new(),
                        };

                        (
                            name.to_string(),
                            ChoiceFieldInfo {
                                points,
                                from,
                                cost,
                                action_options,
                            },
                        )
                    })
                    .collect::<BTreeMap<_, _>>()
            }) else {
                continue;
            };

            // Iterate ACTIONS, not entry.fields — action menus (e.g. Innate
            // Sorcery, Channel Divinity: Read Thoughts) have no same-name
            // runtime field; the field, when it exists, holds stored picks
            // for List-with-non-action options or Ref dropdowns.
            for (action_name, info) in actions.iter() {
                let field_with_index = entry
                    .fields
                    .iter()
                    .enumerate()
                    .find(|(_, field)| field.name == *action_name);

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

                match &info.from {
                    // Action menu or stored choices — both render runtime
                    // options of the matching Choice field. Action items are
                    // marked by `opt.action.is_some()` (mirrored by
                    // `sync_labels` from the registry definition).
                    None => {
                        let Some((_, field)) = field_with_index else {
                            continue;
                        };
                        let FeatureValue::Choice { options } = &field.value else {
                            continue;
                        };
                        let label = field.label().to_string();
                        let items = build_choice_items(
                            options.iter().map(|opt| {
                                // Cast effects aren't mirrored into runtime —
                                // look up by option name in the action's def.
                                let effects = opt
                                    .action
                                    .and_then(|_| {
                                        info.action_options
                                            .iter()
                                            .find(|def_opt| *def_opt.name == opt.name)
                                            .map(|def_opt| def_opt.effects.clone())
                                    })
                                    .unwrap_or_default();
                                ChoiceItemInput {
                                    name: opt.label().to_string(),
                                    description: opt.description.clone(),
                                    cost: opt.cost,
                                    action: opt.action,
                                    effects,
                                    feature_name: feat_name.to_string(),
                                }
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
                    // Ref-based choices (dropdown selects) — render standalone.
                    Some(from) => {
                        let Some((field_index, field)) = field_with_index else {
                            continue;
                        };
                        let FeatureValue::Choice { options } = &field.value else {
                            continue;
                        };
                        let label = field.label().to_string();
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
                        let feat_name = StoredValue::new(feat_name.to_string());

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
                                                        if let Some(entry) = features.get_mut(name.as_str())
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
                                <div class="session-subsection">
                                    <h4 class="session-subsection-title">{label}</h4>
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
                    <div class="session-subsection" style=style>
                        <h4 class="session-subsection-title">{label}</h4>
                        <SessionList items=group.items />
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

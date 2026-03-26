use leptos::{either::EitherOf5, prelude::*};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::{
        datalist_input::DatalistInput, icon::Icon, panel::Panel, toggle_button::ToggleButton,
    },
    model::{
        Character, CharacterIdentityStoreFields, CharacterStoreFields, FeatureOption, FeatureValue,
        Translatable, format_bonus,
    },
    rules::RulesRegistry,
};

#[component]
pub fn ClassFieldsPanels() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<I18n>();

    move || {
        // Extract feature names + field summaries (skip descriptions — read
        // lazily when expanded)
        let entries: Vec<_> = store
            .feature_data()
            .read()
            .iter()
            .filter(|(_, e)| !e.fields.is_empty())
            .map(|(name, e)| (name.clone(), e.fields.clone()))
            .collect();

        entries
            .into_iter()
            .map(|(feature_name, fields)| {
                // Resolve feature name → label for panel title
                let title = registry
                    .with_feature(&feature_name, |f| f.label().to_string())
                    .unwrap_or_else(|| feature_name.clone());
                let fname = StoredValue::new(feature_name);

                let field_views = fields
                    .into_iter()
                    .enumerate()
                    .map(|(field_idx, field)| {
                        let desc = field.description.clone();
                        let field_desc_textarea = move || {
                            view! {
                                <textarea
                                    class="entry-desc"
                                    placeholder=move_tr!("description")
                                    prop:value=desc.clone()
                                    on:change=move |e| {
                                        fname.with_value(|key| {
                                            store.feature_data().update(|m| {
                                                if let Some(fields) = m.get_mut(key).map(|e| &mut e.fields)
                                                    && let Some(f) = fields.get_mut(field_idx)
                                                {
                                                    f.description = event_target_value(&e);
                                                }
                                            });
                                        });
                                    }
                                />
                            }
                        };

                        match &field.value {
                            FeatureValue::Die { die, used } => {
                                let label = field.label().to_string();
                                let die_label = die.to_string();
                                let used_val = used.to_string();
                                let max = die.amount;
                                let max_val = max.to_string();
                                EitherOf5::A(view! {
                                    <div class="entry-item">
                                        <ToggleButton />
                                        <div class="entry-content">
                                            <span class="field-label">{label}" "{die_label}</span>
                                        </div>
                                        <div class="entry-actions" />
                                        <div class="entry-value">
                                            <div class="points-inputs">
                                                <input
                                                    type="number"
                                                    class="short-input"
                                                    min="0"
                                                    prop:max=max_val
                                                    placeholder=move_tr!("used")
                                                    prop:value=used_val
                                                    on:input=move |e| {
                                                        if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                            fname.with_value(|key| {
                                                                store.feature_data().update(|m| {
                                                                    if let Some(fields) = m.get_mut(key).map(|e| &mut e.fields)
                                                                        && let Some(f) = fields.get_mut(field_idx)
                                                                        && let FeatureValue::Die { used, .. } = &mut f.value
                                                                    {
                                                                        *used = value.min(max);
                                                                    }
                                                                });
                                                            });
                                                        }
                                                    }
                                                />
                                                <span>"/" {max}</span>
                                            </div>
                                        </div>
                                        {field_desc_textarea()}
                                    </div>
                                })
                            }
                            FeatureValue::Bonus(val) => {
                                let label = field.label().to_string();
                                let formatted = format_bonus(*val);
                                EitherOf5::B(view! {
                                    <div class="entry-item">
                                        <ToggleButton />
                                        <div class="entry-content">
                                            <span class="field-label">{label}</span>
                                        </div>
                                        <div class="entry-actions" />
                                        <div class="entry-value">
                                            <span class="field-value">{formatted}</span>
                                        </div>
                                        {field_desc_textarea()}
                                    </div>
                                })
                            }
                            FeatureValue::Points { used, max } => {
                                let label = field.label().to_string();
                                let used_val = used.to_string();
                                let max_val = max.to_string();
                                EitherOf5::C(view! {
                                    <div class="entry-item">
                                        <ToggleButton />
                                        <div class="entry-content">
                                            <span class="field-label">{label}</span>
                                        </div>
                                        <div class="entry-actions" />
                                        <div class="entry-value">
                                            <div class="points-inputs">
                                                <input
                                                    type="number"
                                                    class="short-input"
                                                    min="0"
                                                    placeholder=move_tr!("used")
                                                    prop:value=used_val
                                                    on:input=move |e| {
                                                        if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                            fname.with_value(|key| {
                                                                store.feature_data().update(|m| {
                                                                    if let Some(fields) = m.get_mut(key).map(|e| &mut e.fields)
                                                                        && let Some(f) = fields.get_mut(field_idx)
                                                                        && let FeatureValue::Points { used, .. } = &mut f.value
                                                                    {
                                                                        *used = value;
                                                                    }
                                                                });
                                                            });
                                                        }
                                                    }
                                                />
                                                <span>"/"</span>
                                                <input
                                                    type="number"
                                                    class="short-input"
                                                    min="0"
                                                    placeholder=move_tr!("max")
                                                    prop:value=max_val
                                                    on:input=move |e| {
                                                        if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                            fname.with_value(|key| {
                                                                store.feature_data().update(|m| {
                                                                    if let Some(fields) = m.get_mut(key).map(|e| &mut e.fields)
                                                                        && let Some(f) = fields.get_mut(field_idx)
                                                                        && let FeatureValue::Points { max, .. } = &mut f.value
                                                                    {
                                                                        *max = value;
                                                                    }
                                                                });
                                                            });
                                                        }
                                                    }
                                                />
                                            </div>
                                        </div>
                                        {field_desc_textarea()}
                                    </div>
                                })
                            }
                            FeatureValue::Choice { options } => {
                                let label = field.label().to_string();
                                let field_name = field.name.clone();

                                let classes = store.identity().classes().read();
                                let feature_data = store.feature_data().read();
                                let (cost_label, all_options) = fname.with_value(|key| {
                                    let cost_label = registry.get_choice_cost_label(
                                        key,
                                        &field_name,
                                    );
                                    let char_fields = feature_data
                                        .get(key)
                                        .map(|e| e.fields.as_slice())
                                        .unwrap_or(&[]);
                                    let all_options = registry
                                        .get_choice_options(&classes, key, &field_name, char_fields);
                                    (cost_label, all_options)
                                });
                                drop(feature_data);
                                drop(classes);

                                // Action menu: empty stored options + definition
                                // options with `action` → read-only list
                                let char_level = store.read_untracked().level();
                                let is_action_menu = options.is_empty()
                                    && all_options.iter().any(|o| o.action.is_some());

                                if is_action_menu {
                                    let label_view = if let Some(ref cost_title) = cost_label {
                                        format!("{label} ({cost_title})")
                                    } else {
                                        label
                                    };

                                    let action_views = all_options
                                        .iter()
                                        .filter(|opt| opt.level <= char_level)
                                        .map(|opt| {
                                            let action_icon = opt.action.map(|a| {
                                                let title = i18n.tr(a.tr_key());
                                                view! {
                                                    <Icon name=a.icon_name() size=14 title=title />
                                                }
                                            });
                                            let cost_str = (opt.cost > 0).then(|| format!(" ({})", opt.cost));
                                            view! {
                                                <div class="entry-item choice-entry-readonly">
                                                    <div class="entry-content">
                                                        {action_icon}
                                                        <span class="entry-name">{opt.label().to_string()}</span>
                                                    </div>
                                                    <div class="entry-actions">
                                                        {cost_str}
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect_view();

                                    return EitherOf5::E(view! {
                                        <div class="entry-item">
                                            <ToggleButton />
                                            <div class="entry-content">
                                                <span class="field-label">{label_view}</span>
                                            </div>
                                            <div class="entry-actions" />
                                            {field_desc_textarea()}
                                            <div class="entry-list" style="grid-column: 1 / -1">
                                                {action_views}
                                            </div>
                                        </div>
                                    });
                                }

                                let has_cost = cost_label.is_some();
                                let fld_name = StoredValue::new(field_name.clone());
                                let suggestions: Signal<Vec<(String, String, String)>> =
                                    Signal::stored(all_options
                                        .iter()
                                        .map(|o| (o.name.clone(), o.label().to_string(), o.description.clone()))
                                        .collect());

                                let option_views = options
                                    .iter()
                                    .enumerate()
                                    .map(|(opt_idx, option)| {
                                        let opt_name = option.label().to_string();
                                        let opt_cost = option.cost.to_string();
                                        let opt_desc = option.description.clone();

                                        view! {
                                            <div class="entry-item">
                                                <ToggleButton />
                                                <div class="entry-content">
                                                    <DatalistInput
                                                        value=opt_name
                                                        placeholder=move_tr!("choose-option")
                                                        class="entry-name"
                                                        options=suggestions
                                                        on_input=move |input, resolved| {
                                                            fname.with_value(|key| {
                                                                fld_name.with_value(|fld| {
                                                                    let (cost, opt_label, opt_description) = resolved
                                                                        .as_ref()
                                                                        .map(|name| {
                                                                            let classes = store.identity().classes().read();
                                                                            let data = store.feature_data().read();
                                                                            let char_fields = data.get(key)
                                                                                .map(|e| e.fields.as_slice())
                                                                                .unwrap_or(&[]);
                                                                            registry
                                                                                .get_choice_options(&classes, key, fld, char_fields)
                                                                                .into_iter()
                                                                                .find(|o| o.name == *name)
                                                                                .map(|o| (o.cost, o.label, o.description))
                                                                                .map(|(c, l, d)| (Some(c), Some(l), Some(d)))
                                                                                .unwrap_or_default()
                                                                        })
                                                                        .unwrap_or_default();
                                                                    store.feature_data().update(|m| {
                                                                        if let Some(fields) = m.get_mut(key).map(|e| &mut e.fields)
                                                                            && let Some(f) = fields.get_mut(field_idx)
                                                                            && let FeatureValue::Choice { options } = &mut f.value
                                                                            && let Some(opt) = options.get_mut(opt_idx)
                                                                        {
                                                                            if let Some(key) = resolved {
                                                                                opt.name = key;
                                                                                opt.label = opt_label.flatten();
                                                                            } else {
                                                                                opt.set_label(input);
                                                                            }
                                                                            opt.description = opt_description.unwrap_or_default();
                                                                            if let Some(cost) = cost {
                                                                                opt.cost = cost;
                                                                            }
                                                                        }
                                                                    });
                                                                });
                                                            });
                                                        }
                                                    />
                                                    {(has_cost).then(move || view! {
                                                        <input
                                                            type="number"
                                                            class="choice-cost"
                                                            min="0"
                                                            prop:value=opt_cost.clone()
                                                            on:input=move |e| {
                                                                if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                                    fname.with_value(|key| {
                                                                        store.feature_data().update(|m| {
                                                                            if let Some(fields) = m.get_mut(key).map(|e| &mut e.fields)
                                                                                && let Some(f) = fields.get_mut(field_idx)
                                                                                && let FeatureValue::Choice { options } = &mut f.value
                                                                                && let Some(opt) = options.get_mut(opt_idx)
                                                                            {
                                                                                opt.cost = value;
                                                                            }
                                                                        });
                                                                    });
                                                                }
                                                            }
                                                        />
                                                    })}
                                                </div>
                                                <div class="entry-actions">
                                                    <button
                                                        class="btn-remove"
                                                        on:click=move |_| {
                                                            fname.with_value(|key| {
                                                                store.feature_data().update(|m| {
                                                                    if let Some(fields) = m.get_mut(key).map(|e| &mut e.fields)
                                                                        && let Some(f) = fields.get_mut(field_idx)
                                                                        && let FeatureValue::Choice { options } = &mut f.value
                                                                        && opt_idx < options.len()
                                                                    {
                                                                        options.remove(opt_idx);
                                                                    }
                                                                });
                                                            });
                                                        }
                                                    >
                                                        <Icon name="x" size=14 />
                                                    </button>
                                                </div>
                                                <textarea
                                                    class="entry-desc"
                                                    placeholder=move_tr!("description")
                                                    prop:value=opt_desc.clone()
                                                    on:change=move |e| {
                                                        fname.with_value(|key| {
                                                            store.feature_data().update(|m| {
                                                                if let Some(fields) = m.get_mut(key).map(|e| &mut e.fields)
                                                                    && let Some(f) = fields.get_mut(field_idx)
                                                                    && let FeatureValue::Choice { options } = &mut f.value
                                                                    && let Some(opt) = options.get_mut(opt_idx)
                                                                {
                                                                    opt.description = event_target_value(&e);
                                                                }
                                                            });
                                                        });
                                                    }
                                                />
                                            </div>
                                        }
                                    })
                                    .collect_view();

                                // description read lazily below

                                let label_view = if let Some(ref cost_title) = cost_label {
                                    format!("{label} ({cost_title})")
                                } else {
                                    label
                                };

                                EitherOf5::D(view! {
                                    <div class="entry-item">
                                        <ToggleButton />
                                        <div class="entry-content">
                                            <span class="field-label">{label_view}</span>
                                        </div>
                                        <div class="entry-actions" />
                                        {field_desc_textarea()}
                                        <div class="entry-list" style="grid-column: 1 / -1">
                                            {option_views}
                                        </div>
                                    </div>
                                    <button
                                        class="btn-primary"
                                        on:click=move |_| {
                                            fname.with_value(|key| {
                                                store.feature_data().update(|m| {
                                                    if let Some(fields) = m.get_mut(key).map(|e| &mut e.fields)
                                                        && let Some(f) = fields.get_mut(field_idx)
                                                        && let FeatureValue::Choice { options } = &mut f.value
                                                    {
                                                        options.push(FeatureOption::default());
                                                    }
                                                });
                                            });
                                        }
                                    >
                                        {move_tr!("btn-add-option")}
                                    </button>
                                })
                            }
                        }
                    })
                    .collect_view();

                view! {
                    <Panel title=title class="class-fields-panel">
                        <div class="entry-list">
                            {field_views}
                        </div>
                    </Panel>
                }
            })
            .collect_view()
    }
}

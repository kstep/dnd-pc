use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{datalist_input::DatalistInput, panel::Panel, toggle_button::ToggleButton},
    model::{
        Character, CharacterIdentityStoreFields, CharacterStoreFields, FeatureOption, FeatureValue,
    },
    rules::RulesRegistry,
};

#[component]
pub fn ClassFieldsPanels() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    move || {
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
                let title = Signal::derive({
                    let feature_name = feature_name.clone();
                    move || feature_name.clone()
                });

                let desc_expanded = RwSignal::new(HashSet::<usize>::new());

                let all_fields = fields.clone();
                let field_views = fields
                    .into_iter()
                    .enumerate()
                    .map(|(field_idx, field)| {
                        let feature_name = feature_name.clone();
                        let desc = field.description.clone();
                        let is_open =
                            Signal::derive(move || desc_expanded.get().contains(&field_idx));

                        match &field.value {
                            FeatureValue::Die(die_str) => {
                                let label = field.name.clone();
                                let die = die_str.clone();
                                let d = desc.clone();
                                let fname = feature_name.clone();
                                view! {
                                    <div class="field-entry">
                                        <ToggleButton
                                            expanded=is_open
                                            on_toggle=move || desc_expanded.update(|set| {
                                                if !set.remove(&field_idx) { set.insert(field_idx); }
                                            })
                                        />
                                        <span class="field-label">{label}</span>
                                        <span class="field-value">{die}</span>
                                        <Show when=move || is_open.get()>
                                            <textarea
                                                class="field-desc"
                                                placeholder=move_tr!("description")
                                                prop:value=d.clone()
                                                on:change={
                                                    let fname = fname.clone();
                                                    move |e| {
                                                        let fname = fname.clone();
                                                        store.feature_data().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                                && let Some(f) = fields.get_mut(field_idx)
                                                            {
                                                                f.description = event_target_value(&e);
                                                            }
                                                        });
                                                    }
                                                }
                                            />
                                        </Show>
                                    </div>
                                }
                                .into_any()
                            }
                            FeatureValue::Bonus(val) => {
                                let label = field.name.clone();
                                let formatted = if *val >= 0 {
                                    format!("+{val}")
                                } else {
                                    format!("{val}")
                                };
                                let d = desc.clone();
                                let fname = feature_name.clone();
                                view! {
                                    <div class="field-entry">
                                        <ToggleButton
                                            expanded=is_open
                                            on_toggle=move || desc_expanded.update(|set| {
                                                if !set.remove(&field_idx) { set.insert(field_idx); }
                                            })
                                        />
                                        <span class="field-label">{label}</span>
                                        <span class="field-value">{formatted}</span>
                                        <Show when=move || is_open.get()>
                                            <textarea
                                                class="field-desc"
                                                placeholder=move_tr!("description")
                                                prop:value=d.clone()
                                                on:change={
                                                    let fname = fname.clone();
                                                    move |e| {
                                                        let fname = fname.clone();
                                                        store.feature_data().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                                && let Some(f) = fields.get_mut(field_idx)
                                                            {
                                                                f.description = event_target_value(&e);
                                                            }
                                                        });
                                                    }
                                                }
                                            />
                                        </Show>
                                    </div>
                                }
                                .into_any()
                            }
                            FeatureValue::Points { used, max } => {
                                let label = field.name.clone();
                                let used_val = used.to_string();
                                let max_val = max.to_string();
                                let fname = feature_name.clone();
                                let fname2 = feature_name.clone();
                                let fname3 = feature_name.clone();
                                let d = desc.clone();
                                view! {
                                    <div class="field-entry field-points">
                                        <ToggleButton
                                            expanded=is_open
                                            on_toggle=move || desc_expanded.update(|set| {
                                                if !set.remove(&field_idx) { set.insert(field_idx); }
                                            })
                                        />
                                        <span class="field-label">{label}</span>
                                        <div class="points-inputs">
                                            <input
                                                type="number"
                                                class="short-input"
                                                min="0"
                                                placeholder=move_tr!("used")
                                                prop:value=used_val
                                                on:input=move |e| {
                                                    if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                        let fname = fname.clone();
                                                        store.feature_data().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                                && let Some(f) = fields.get_mut(field_idx)
                                                                && let FeatureValue::Points { used, .. } = &mut f.value
                                                            {
                                                                *used = v;
                                                            }
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
                                                    if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                        let fname = fname2.clone();
                                                        store.feature_data().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                                && let Some(f) = fields.get_mut(field_idx)
                                                                && let FeatureValue::Points { max, .. } = &mut f.value
                                                            {
                                                                *max = v;
                                                            }
                                                        });
                                                    }
                                                }
                                            />
                                        </div>
                                        <Show when=move || is_open.get()>
                                            <textarea
                                                class="field-desc"
                                                placeholder=move_tr!("description")
                                                prop:value=d.clone()
                                                on:change={
                                                    let fname = fname3.clone();
                                                    move |e| {
                                                        let fname = fname.clone();
                                                        store.feature_data().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                                && let Some(f) = fields.get_mut(field_idx)
                                                            {
                                                                f.description = event_target_value(&e);
                                                            }
                                                        });
                                                    }
                                                }
                                            />
                                        </Show>
                                    </div>
                                }
                                .into_any()
                            }
                            FeatureValue::Choice { options } => {
                                let label = field.name.clone();
                                let field_name = field.name.clone();

                                let classes = store.identity().classes().read();
                                let cost_label = registry.get_choice_cost_label(
                                    &classes,
                                    &feature_name,
                                    &field_name,
                                );
                                let all_options = registry
                                    .get_choice_options(&classes, &feature_name, &field_name, &all_fields);
                                drop(classes);

                                let opt_expanded = RwSignal::new(HashSet::<usize>::new());
                                let has_cost = cost_label.is_some();

                                let option_views = options
                                    .iter()
                                    .enumerate()
                                    .map(|(opt_idx, option)| {
                                        let fname = feature_name.clone();
                                        let fname2 = feature_name.clone();
                                        let fname3 = feature_name.clone();
                                        let fname4 = feature_name.clone();
                                        let fld_name = field_name.clone();
                                        let opt_name = option.name.clone();
                                        let opt_desc = option.description.clone();
                                        let opt_cost = option.cost.to_string();
                                        let is_opt_open = Signal::derive(move || {
                                            opt_expanded.get().contains(&opt_idx)
                                        });

                                        let suggestions: Vec<(String, String)> = all_options
                                            .iter()
                                            .map(|o| (o.name.clone(), o.description.clone()))
                                            .collect();

                                        view! {
                                            <div class="choice-entry">
                                                <ToggleButton
                                                    expanded=is_opt_open
                                                    on_toggle=move || opt_expanded.update(|set| {
                                                        if !set.remove(&opt_idx) { set.insert(opt_idx); }
                                                    })
                                                />
                                                <DatalistInput
                                                    value=opt_name
                                                    placeholder=move_tr!("choose-option").get()
                                                    options=suggestions
                                                    on_input=move |val: String| {
                                                        let classes = store.identity().classes().read();
                                                        let char_fields: Vec<_> = store.feature_data().read()
                                                            .get(&fname)
                                                            .map(|e| e.fields.clone())
                                                            .unwrap_or_default();
                                                        let found = registry
                                                            .get_choice_options(&classes, &fname, &fld_name, &char_fields)
                                                            .into_iter()
                                                            .find(|o| o.name == val);
                                                        drop(classes);
                                                        let fname = fname.clone();
                                                        store.feature_data().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                                && let Some(f) = fields.get_mut(field_idx)
                                                                && let FeatureValue::Choice { options } = &mut f.value
                                                                && let Some(opt) = options.get_mut(opt_idx)
                                                            {
                                                                opt.name = val.clone();
                                                                if let Some(ref o) = found {
                                                                    opt.description = o.description.clone();
                                                                    opt.cost = o.cost;
                                                                }
                                                            }
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
                                                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                                let fname = fname2.clone();
                                                                store.feature_data().update(|m| {
                                                                    if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                                        && let Some(f) = fields.get_mut(field_idx)
                                                                        && let FeatureValue::Choice { options } = &mut f.value
                                                                        && let Some(opt) = options.get_mut(opt_idx)
                                                                    {
                                                                        opt.cost = v;
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    />
                                                })}
                                                <button
                                                    class="btn-remove"
                                                    on:click=move |_| {
                                                        let fname = fname3.clone();
                                                        store.feature_data().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                                && let Some(f) = fields.get_mut(field_idx)
                                                                && let FeatureValue::Choice { options } = &mut f.value
                                                                && opt_idx < options.len()
                                                            {
                                                                options.remove(opt_idx);
                                                            }
                                                        });
                                                    }
                                                >
                                                    "X"
                                                </button>
                                                <Show when=move || is_opt_open.get()>
                                                    <textarea
                                                        class="choice-desc"
                                                        placeholder=move_tr!("description")
                                                        prop:value=opt_desc.clone()
                                                        on:change={
                                                            let fname = fname4.clone();
                                                            move |e| {
                                                                let fname = fname.clone();
                                                                store.feature_data().update(|m| {
                                                                    if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                                        && let Some(f) = fields.get_mut(field_idx)
                                                                        && let FeatureValue::Choice { options } = &mut f.value
                                                                        && let Some(opt) = options.get_mut(opt_idx)
                                                                    {
                                                                        opt.description = event_target_value(&e);
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    />
                                                </Show>
                                            </div>
                                        }
                                    })
                                    .collect_view();

                                let d = desc.clone();
                                let fname_desc = feature_name.clone();
                                let fname_add = feature_name.clone();

                                let label_view = if let Some(ref cost_title) = cost_label {
                                    format!("{label} ({cost_title})")
                                } else {
                                    label
                                };

                                view! {
                                    <div class="field-entry field-choice">
                                        <div class="field-header">
                                            <ToggleButton
                                                expanded=is_open
                                                on_toggle=move || desc_expanded.update(|set| {
                                                    if !set.remove(&field_idx) { set.insert(field_idx); }
                                                })
                                            />
                                            <span class="field-label">{label_view}</span>
                                        </div>
                                        <Show when=move || is_open.get()>
                                            <textarea
                                                class="field-desc"
                                                placeholder=move_tr!("description")
                                                prop:value=d.clone()
                                                on:change={
                                                    let fname = fname_desc.clone();
                                                    move |e| {
                                                        let fname = fname.clone();
                                                        store.feature_data().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                                && let Some(f) = fields.get_mut(field_idx)
                                                            {
                                                                f.description = event_target_value(&e);
                                                            }
                                                        });
                                                    }
                                                }
                                            />
                                        </Show>
                                        <div class="choice-list">
                                            {option_views}
                                            <button
                                                class="btn-add"
                                                on:click=move |_| {
                                                    let fname = fname_add.clone();
                                                    store.feature_data().update(|m| {
                                                        if let Some(fields) = m.get_mut(&fname).map(|e| &mut e.fields)
                                                            && let Some(f) = fields.get_mut(field_idx)
                                                            && let FeatureValue::Choice { options } = &mut f.value
                                                        {
                                                            options.push(FeatureOption::default());
                                                        }
                                                    });
                                                }
                                            >
                                                {move_tr!("btn-add-option")}
                                            </button>
                                        </div>
                                    </div>
                                }
                                .into_any()
                            }
                        }
                    })
                    .collect_view();

                view! {
                    <Panel title=title class="class-fields-panel">
                        <div class="class-fields-list">
                            {field_views}
                        </div>
                    </Panel>
                }
            })
            .collect_view()
    }
}

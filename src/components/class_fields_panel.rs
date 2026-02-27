use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{datalist_input::DatalistInput, panel::Panel},
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields, FeatureValue},
    rules::RulesRegistry,
};

#[component]
pub fn ClassFieldsPanels() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    move || {
        let entries: Vec<_> = store
            .fields()
            .read()
            .iter()
            .map(|(name, fields)| (name.clone(), fields.clone()))
            .collect();

        entries
            .into_iter()
            .map(|(feature_name, fields)| {
                let title = Signal::derive({
                    let feature_name = feature_name.clone();
                    move || feature_name.clone()
                });

                let field_views = fields
                    .into_iter()
                    .enumerate()
                    .map(|(field_idx, field)| {
                        let feature_name = feature_name.clone();
                        match &field.value {
                            FeatureValue::Die(die_str) => {
                                let label = field.name.clone();
                                let die = die_str.clone();
                                view! {
                                    <div class="field-entry">
                                        <span class="field-label">{label}</span>
                                        <span class="field-value">{die}</span>
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
                                view! {
                                    <div class="field-entry">
                                        <span class="field-label">{label}</span>
                                        <span class="field-value">{formatted}</span>
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
                                view! {
                                    <div class="field-entry field-points">
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
                                                        store.fields().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname)
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
                                                        store.fields().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname)
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
                                    </div>
                                }
                                .into_any()
                            }
                            FeatureValue::Choice { options } => {
                                let label = field.name.clone();
                                let field_name = field.name.clone();

                                let option_views = options
                                    .iter()
                                    .enumerate()
                                    .map(|(opt_idx, option)| {
                                        let fname = feature_name.clone();
                                        let fld_name = field_name.clone();
                                        let opt_name = option.name.clone();
                                        let opt_desc = option.description.clone();

                                        let classes = store.identity().classes().read();
                                        let suggestions: Vec<(String, String)> = registry
                                            .get_choice_options(&classes, &feature_name, &field_name)
                                            .into_iter()
                                            .map(|o| (o.name.clone(), o.description.clone()))
                                            .collect();
                                        drop(classes);

                                        view! {
                                            <div class="choice-entry">
                                                <DatalistInput
                                                    value=opt_name
                                                    placeholder=move_tr!("choose-option").get()
                                                    options=suggestions
                                                    on_input=move |val: String| {
                                                        let classes = store.identity().classes().read();
                                                        let desc = registry
                                                            .get_choice_options(&classes, &fname, &fld_name)
                                                            .into_iter()
                                                            .find(|o| o.name == val)
                                                            .map(|o| o.description.clone());
                                                        drop(classes);
                                                        let fname = fname.clone();
                                                        store.fields().update(|m| {
                                                            if let Some(fields) = m.get_mut(&fname)
                                                                && let Some(f) = fields.get_mut(field_idx)
                                                                && let FeatureValue::Choice { options } = &mut f.value
                                                                && let Some(opt) = options.get_mut(opt_idx)
                                                            {
                                                                opt.name = val.clone();
                                                                if let Some(d) = desc.clone() {
                                                                    opt.description = d;
                                                                }
                                                            }
                                                        });
                                                    }
                                                />
                                                {(!opt_desc.is_empty()).then(|| view! {
                                                    <p class="choice-desc">{opt_desc}</p>
                                                })}
                                            </div>
                                        }
                                    })
                                    .collect_view();

                                view! {
                                    <div class="field-entry field-choice">
                                        <span class="field-label">{label}</span>
                                        <div class="choice-list">
                                            {option_views}
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

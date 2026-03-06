use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    model::{Character, CharacterIdentity, CharacterStoreFields, FeatureValue},
    rules::{ChoiceOptions, FieldKind, RulesRegistry},
};

fn is_choice_ref(
    registry: &RulesRegistry,
    identity: &CharacterIdentity,
    feat_name: &str,
    field_name: &str,
) -> bool {
    registry
        .with_feature(identity, feat_name, |feat| {
            feat.fields.get(field_name).is_some_and(|fd| {
                matches!(&fd.kind, FieldKind::Choice { options, .. } if matches!(options, ChoiceOptions::Ref { .. }))
            })
        })
        .unwrap_or(false)
}

#[component]
pub fn ChoiceRefsBlock() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let store = expect_context::<Store<Character>>();

    let identity = store.identity();
    let ref_choices = store
        .feature_data()
        .read()
        .iter()
        .flat_map(|(feat_name, entry)| {
            let all_fields = entry.fields.clone();
            entry
                .fields
                .iter()
                .enumerate()
                .filter_map(|(field_idx, field)| {
                    if let FeatureValue::Choice { .. } = &field.value
                        && is_choice_ref(&registry, &identity.read(), feat_name, &field.name)
                    {
                        Some((
                            feat_name.clone(),
                            field_idx,
                            field.label().to_string(),
                            field.name.clone(),
                            all_fields.clone(),
                        ))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    if ref_choices.is_empty() {
        return None;
    }

    Some(
        ref_choices
            .into_iter()
            .map(|(feat_name, field_idx, label, field_name, all_fields)| {
                let fname = StoredValue::new(feat_name);

                let classes = &*identity.read().classes;
                let (cost_label, all_options) = fname.with_value(|key| {
                    let cost_label = registry.get_choice_cost_label(classes, key, &field_name);
                    let all_options =
                        registry.get_choice_options(classes, key, &field_name, &all_fields);
                    (cost_label, all_options)
                });

                let all_options = StoredValue::new(all_options);

                let options = store
                    .feature_data()
                    .read()
                    .get(&fname.get_value())
                    .and_then(|e| e.fields.get(field_idx))
                    .map(|f| f.value.choices().to_vec())
                    .unwrap_or_default();

                let option_views = options
                    .iter()
                    .enumerate()
                    .map(|(opt_idx, option)| {
                        let selected_name = option.name.clone();

                        view! {
                            <div class="choice-entry">
                                <select
                                    on:change=move |e| {
                                        let value = event_target_value(&e);
                                        fname.with_value(|key| {
                                            let cost = all_options.with_value(|opts| {
                                                opts.iter()
                                                    .find(|o| o.name == value)
                                                    .map(|o| o.cost)
                                            });
                                            store.feature_data().update(|m| {
                                                if let Some(fields) = m.get_mut(key).map(|e| &mut e.fields)
                                                    && let Some(f) = fields.get_mut(field_idx)
                                                    && let FeatureValue::Choice { options } = &mut f.value
                                                    && let Some(opt) = options.get_mut(opt_idx)
                                                {
                                                    opt.name = value.clone();
                                                    opt.label = None;
                                                    opt.description.clear();
                                                    if let Some(cost) = cost {
                                                        opt.cost = cost;
                                                    }
                                                }
                                            });
                                        });
                                    }
                                >
                                    <option value="" selected=selected_name.is_empty()>""</option>
                                    {all_options.with_value(|opts| {
                                        opts.iter().map(|o| {
                                            let name = o.name.clone();
                                            let label = o.label().to_string();
                                            let is_selected = name == selected_name;
                                            view! {
                                                <option value=name selected=is_selected>{label}</option>
                                            }
                                        }).collect_view()
                                    })}
                                </select>
                            </div>
                        }
                    })
                    .collect_view();

                let label_view = if let Some(ref cost_title) = cost_label {
                    format!("{label} ({cost_title})")
                } else {
                    label
                };

                view! {
                    <h4 class="summary-subsection-title">{label_view}</h4>
                    <div class="choice-list">
                        {option_views}
                    </div>
                }
            })
            .collect_view(),
    )
}

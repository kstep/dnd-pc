use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::components::A;
use reactive_stores::Store;

use crate::{
    BASE_URL,
    components::{
        apply::apply_with_modal, datalist_input::DatalistInput, feature_field_row::FeatureFieldRow,
        icon::Icon,
    },
    model::{Character, CharacterStoreFields, FeatureSource, FeatureValue},
    rules::{
        RulesRegistry,
        apply::{PendingFeature, apply_new_features},
    },
};

#[component]
pub fn FeatureRow(
    feature_idx: usize,
    options: Memo<Vec<(String, String, String)>>,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let features = store.features();

    let i = feature_idx;
    let feature = features.read_untracked().get(i).cloned()?;

    let name = feature.label().to_string();
    let desc = feature.description.clone();
    let feature_name = feature.name.clone();
    let source = feature.source.clone();
    let is_readonly = !matches!(source, FeatureSource::User(_))
        || registry.with_features_index_untracked(|idx| idx.contains_key(feature_name.as_str()));
    let stored_name = StoredValue::new(feature_name.clone());
    let (field_count, has_spells, has_empty_choices) = store
        .feature_data()
        .read_untracked()
        .get(&feature_name)
        .map(|e| {
            let has_empty = e.fields.iter().any(|f| {
                matches!(
                    &f.value,
                    FeatureValue::Choice { options } if options.iter().any(|o| o.name.is_empty())
                )
            });
            (e.fields.len(), e.spells.is_some(), has_empty)
        })
        .unwrap_or((0, false, false));
    let fname = feature_name.clone();
    let has_pending = Memo::new(move |_| {
        store
            .feature_data()
            .read()
            .get(&fname)
            .map(|e| {
                e.fields.iter().any(|f| {
                    matches!(
                        &f.value,
                        FeatureValue::Choice { options }
                            if options.iter().any(|o| o.name.is_empty())
                    )
                })
            })
            .unwrap_or(false)
    });
    let fname2 = feature_name.clone();
    let badges = move || {
        store
            .feature_data()
            .read()
            .get(&fname2)
            .map(|e| {
                let (choice, points, die) =
                    e.fields
                        .iter()
                        .fold((0u32, 0u32, 0u32), |(c, p, d), f| match &f.value {
                            FeatureValue::Choice { .. } => (c + 1, p, d),
                            FeatureValue::Points { .. } => (c, p + 1, d),
                            FeatureValue::Die { .. } => (c, p, d + 1),
                            _ => (c, p, d),
                        });
                [
                    ("list-checks", choice),
                    ("circle-dot", points),
                    ("dices", die),
                ]
                .into_iter()
                .filter(|(_, n)| *n > 0)
                .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let spell_link = has_spells.then(|| {
        let char_id = store.read_untracked().id;
        format!("{BASE_URL}/c/{char_id}/magic#{feature_name}")
    });

    let expanded = RwSignal::new(has_empty_choices);
    let anchor_id = feature_name.clone();
    Some(view! {
        <div
            id=anchor_id
            class="entry-item"
            class:expanded=move || expanded.get()
            class:has-pending=move || has_pending.get()
        >
            <button
                class="btn-toggle-desc"
                on:click=move |_| expanded.update(|v| *v = !*v)
            />
            <div class="entry-content">
                {if is_readonly {
                    Either::Left(view! {
                        <span class="entry-name entry-name-readonly">{name.clone()}</span>
                    })
                } else {
                    Either::Right(view! {
                        <DatalistInput
                            value=name
                            placeholder=move_tr!("feature-name")
                            class="entry-name"
                            options=options
                            on_input=move |input, resolved| {
                                let mut w = features.write();
                                if let Some(key) = resolved {
                                    w[i].name = key.clone();
                                    let (label, description) =
                                        registry.with_features_index(|idx| {
                                            idx.get(key.as_str())
                                                .map(|feat| {
                                                    (
                                                        feat.label.clone(),
                                                        feat.description.clone(),
                                                    )
                                                })
                                                .unwrap_or_default()
                                        });
                                    w[i].label = label;
                                    w[i].description = description;
                                } else {
                                    w[i].set_label(input);
                                    w[i].description.clear();
                                }
                            }
                        />
                    })
                }}
                {move || badges()
                    .into_iter()
                    .map(|(icon, n)| view! {
                        <span class="entry-badge">
                            <Icon name=icon />
                            {n}
                        </span>
                    })
                    .collect_view()}
                {spell_link.map(|href| view! {
                    <A href=href scroll=false attr:class="entry-spell-link">
                        {move_tr!("tab-magic")}" →"
                    </A>
                })}
            </div>
            <div class="entry-actions">
                <button
                    class="btn-apply-level"
                    title=move_tr!("btn-apply-feature")
                    on:click=move |_| {
                        let name = features.read()[i].name.clone();
                        let level = store.with_untracked(|character| {
                            registry
                                .feature_class_level(&character.identity, &name)
                                .unwrap_or_else(|| character.level())
                        });
                        let pending = vec![PendingFeature {
                            name,
                            source: FeatureSource::User(level),
                            level,
                        }];
                        apply_with_modal(
                            store,
                            registry,
                            pending,
                            move |character, pending, inputs, fi| {
                                apply_new_features(fi, character, pending, Some(inputs));
                            },
                        );
                    }
                >
                    <Icon name="arrow-up" />
                </button>
                <button
                    class="btn-remove"
                    on:click=move |_| {
                        if i < features.read().len() {
                            let removed = features.write().remove(i);
                            if !features.read().iter().any(|f| f.name == removed.name) {
                                store.feature_data().write().remove(&removed.name);
                            }
                        }
                    }
                >
                    <Icon name="x" />
                </button>
            </div>
            {if is_readonly {
                Either::Left(view! {
                    <p class="entry-desc">{desc.clone()}</p>
                })
            } else {
                Either::Right(view! {
                    <textarea
                        class="entry-desc"
                        placeholder=move_tr!("description")
                        prop:value=desc.clone()
                        on:change=move |e| {
                            features.write()[i].description = event_target_value(&e);
                        }
                    />
                })
            }}
            {(field_count > 0).then(move || view! {
                <div class="feature-fields" style="grid-column: 1 / -1">
                    {(0..field_count)
                        .map(|field_idx| view! {
                            <FeatureFieldRow feature_name=stored_name field_idx=field_idx />
                        })
                        .collect_view()}
                </div>
            })}
        </div>
    })
}

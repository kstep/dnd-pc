use std::sync::Arc;

use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        apply::{apply_with_modal, edit_inputs_modal},
        datalist_input::{DatalistInput, DatalistOption},
        feature_field_row::FeatureFieldRow,
        icon::Icon,
        ref_link::Ref,
    },
    model::{Character, CharacterStoreFields, FeatureValue, FeaturesStoreFields},
    rules::{
        RulesRegistry,
        apply::{FeatureKey, PendingFeature, apply_new_features, build_cascade_base_before},
    },
};

/// Whether the current URL fragment points at `anchor`. Used so the toggle
/// button can decide the initial transition when nothing has been clicked
/// yet — `:target` is showing the row purely through CSS.
fn location_hash_matches(anchor: &str) -> bool {
    web_sys::window()
        .map(|win| win.location())
        .and_then(|loc| loc.hash().ok())
        .map(|hash| hash.trim_start_matches('#') == anchor)
        .unwrap_or(false)
}

#[component]
pub fn FeatureRow(
    feature_idx: usize,
    options: Memo<Vec<DatalistOption>>,
    assign_previews: Memo<Vec<Vec<String>>>,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let features = store.features().list();

    let feature = features.read_untracked().get(feature_idx).cloned()?;

    let name = feature.label().to_string();
    let desc = feature.description.clone();
    let feature_name = feature.name.clone();
    let source = feature.source.clone();
    let is_readonly = !source.is_user()
        || registry.with_features_index_untracked(|idx| idx.contains_key(feature_name.as_str()));
    let stored_name = StoredValue::new(feature_name.clone());
    let (field_count, has_spells, has_empty_choices) = store
        .features()
        .data()
        .read_untracked()
        .get(&feature_name)
        .map(|feature_data| {
            let has_empty = feature_data.fields.iter().any(|field| {
                matches!(
                    &field.value,
                    FeatureValue::Choice { options } if options.iter().any(|opt| opt.label().is_empty())
                )
            });
            (
                feature_data.fields.len(),
                feature_data.spells.is_some(),
                has_empty,
            )
        })
        .unwrap_or((0, false, false));
    // One reactive read of list + data produces both the pending flag and the
    // badge counts. Keeps the FeatureData lock scoped to a single iteration.
    let row_info = Memo::new(move |_| {
        let not_applied = store
            .features()
            .list()
            .read()
            .get(feature_idx)
            .map(|feature| !feature.applied)
            .unwrap_or(false);
        let (has_empty, choices, points, dies) = stored_name
            .with_value(|key| {
                store.features().data().read().get(key).map(|feature_data| {
                    feature_data.fields.iter().fold(
                        (false, 0u32, 0u32, 0u32),
                        |(has_empty, choices, points, dies), field| match &field.value {
                            FeatureValue::Choice { options } => {
                                let empty =
                                    has_empty || options.iter().any(|opt| opt.label().is_empty());
                                (empty, choices + 1, points, dies)
                            }
                            FeatureValue::Points { .. } => (has_empty, choices, points + 1, dies),
                            FeatureValue::Die { .. } => (has_empty, choices, points, dies + 1),
                            _ => (has_empty, choices, points, dies),
                        },
                    )
                })
            })
            .unwrap_or((false, 0, 0, 0));
        let has_pending = not_applied || has_empty;
        let badges = [
            (choices > 0).then_some(("list-checks", choices)),
            (points > 0).then_some(("circle-dot", points)),
            (dies > 0).then_some(("dices", dies)),
        ];
        (has_pending, badges)
    });
    let spell_link = has_spells.then(|| {
        let char_id = store.read_untracked().id;
        format!("/c/{char_id}/magic#{feature_name}")
    });
    let has_interactive_inputs = registry.with_features_index_untracked(|idx| {
        idx.get(feature_name.as_str())
            .is_some_and(|feat_def| feat_def.has_interactive_inputs())
    });

    // `None` means "no explicit user choice yet" — visibility falls back to
    // `:target` (CSS). Once the user clicks the toggle, we pin the row with
    // either `expanded` or `collapsed` class, with `:not(.collapsed)` in the
    // CSS rule keeping `:target` from resurrecting a collapsed row.
    let state: RwSignal<Option<bool>> = RwSignal::new(has_empty_choices.then_some(true));
    let anchor_id = feature.dom_id();
    let toggle_anchor = StoredValue::new(anchor_id.clone());
    let toggle = move || {
        let currently_shown = state
            .get_untracked()
            .unwrap_or_else(|| toggle_anchor.with_value(|anchor| location_hash_matches(anchor)));
        state.set(Some(!currently_shown));
    };
    Some(view! {
        <div
            id=anchor_id
            class="entry-item"
            class:expanded=move || state.get() == Some(true)
            class:collapsed=move || state.get() == Some(false)
            class:has-pending=move || row_info.get().0
        >
            <button
                class="btn-toggle-desc"
                on:click=move |_| toggle()
            />
            <div class="entry-content">
                {if is_readonly {
                    Either::Left(view! {
                        <span
                            class="entry-name entry-name-readonly"
                            on:click=move |_| toggle()
                        >
                            {name.clone()}
                        </span>
                    })
                } else {
                    Either::Right(view! {
                        <DatalistInput
                            value=name
                            placeholder=move_tr!("feature-name")
                            class="entry-name"
                            options=options
                            on_input=move |input, resolved| {
                                let key_for_apply = {
                                    let mut w = features.write();
                                    if let Some(key) = resolved {
                                        w[feature_idx].name = key.clone();
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
                                        w[feature_idx].label = label;
                                        w[feature_idx].description = description;
                                        Some(key)
                                    } else {
                                        w[feature_idx].set_label(input);
                                        w[feature_idx].description.clear();
                                        None
                                    }
                                };
                                // Auto-apply when user picks a non-interactive feat from
                                // the list. Interactive feats stay pending and surface
                                // through the Edit button; they are intentionally NOT
                                // auto-opening the args modal (that felt too aggressive
                                // in review).
                                if let Some(key) = key_for_apply {
                                    let is_non_interactive = registry.with_features_index_untracked(|idx| {
                                        idx.get(key.as_str())
                                            .is_some_and(|feat_def| !feat_def.has_interactive_inputs())
                                    });
                                    if is_non_interactive {
                                        let source = features.read_untracked()[feature_idx].source.clone();
                                        let level = source.added_at_level();
                                        let pending = vec![PendingFeature {
                                            name: key,
                                            source,
                                            level,
                                        }];
                                        apply_with_modal(
                                            store,
                                            registry,
                                            pending,
                                            None,
                                            move |character, pending, inputs, fi| {
                                                apply_new_features(
                                                    fi,
                                                    character,
                                                    pending,
                                                    Some(&inputs.feature_inputs),
                                                );
                                            },
                                        );
                                    }
                                }
                            }
                        />
                    })
                }}
                {move || row_info.get().1
                    .into_iter()
                    .flatten()
                    .map(|(icon, count)| view! {
                        <span class="entry-badge">
                            <Icon name=icon />
                            {count}
                        </span>
                    })
                    .collect_view()}
                {spell_link.map(|href| view! {
                    <Ref href=href scroll=false attr:class="entry-spell-link">
                        {move_tr!("tab-magic")}" →"
                    </Ref>
                })}
            </div>
            <div class="entry-actions">
                <Show when=move || has_interactive_inputs>
                    <button
                        class="btn-apply-level"
                        title=move_tr!("btn-edit-feature")
                        on:click=move |_| {
                            let (name, source, is_applied) = {
                                let feature = &features.read()[feature_idx];
                                (feature.name.clone(), feature.source.clone(), feature.applied)
                            };
                            if is_applied {
                                // Edit-mode: open modal with pre-edit cascade snapshot, on
                                // submit just stash new inputs + mark dirty. Replay banner
                                // picks it up and performs the full-character re-apply.
                                let key = FeatureKey::new(name.clone(), source.clone());
                                let clean = registry.with_features_index_untracked(|fi| {
                                    build_cascade_base_before(fi, &store.read_untracked(), &key)
                                });
                                edit_inputs_modal(
                                    store,
                                    registry,
                                    name,
                                    source,
                                    Some(Arc::new(clean)),
                                );
                            } else {
                                // First-time apply: full apply for interactive feature.
                                let level = source.added_at_level();
                                let pending = vec![PendingFeature { name, source, level }];
                                apply_with_modal(
                                    store,
                                    registry,
                                    pending,
                                    None,
                                    move |character, pending, inputs, fi| {
                                        apply_new_features(
                                            fi,
                                            character,
                                            pending,
                                            Some(&inputs.feature_inputs),
                                        );
                                    },
                                );
                            }
                        }
                    >
                        <Icon name="pencil" />
                    </button>
                </Show>
                <button
                    class="btn-remove"
                    on:click=move |_| {
                        if feature_idx < features.read().len() {
                            let removed = features.write().remove(feature_idx);
                            if !features.read().iter().any(|feature| feature.name == removed.name) {
                                store.features().write().remove(&removed.name);
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
                        on:change=move |event| {
                            features.write()[feature_idx].description =
                                event_target_value(&event);
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
            {move || {
                let entries = assign_previews
                    .with(|previews| previews.get(feature_idx).cloned().unwrap_or_default());
                (!entries.is_empty()).then(|| view! {
                    <div class="entry-full-row feature-assignments">
                        {entries.into_iter().map(|entry| view! {
                            <span class="feature-assignment-entry">{entry}</span>
                        }).collect_view()}
                    </div>
                })
            }}
        </div>
    })
}

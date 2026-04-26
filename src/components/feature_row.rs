use std::sync::Arc;

use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::{Field, Store};

use crate::{
    components::{
        apply::{apply_with_modal, edit_inputs_modal},
        datalist::{DatalistInput, DatalistOption, SharedDatalist, next_datalist_id},
        feature_field_row::FeatureFieldRow,
        icon::Icon,
        markdown::Markdown,
        ref_link::Ref,
    },
    model::{
        Character, CharacterStoreFields, Feature, FeatureStoreFields, FeatureValue,
        FeaturesStoreFields,
    },
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
    feature: Field<Feature>,
    options: Memo<Vec<DatalistOption>>,
    row_previews: Signal<Vec<String>>,
    on_remove: Callback<()>,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    let (feature_name, initial_label, initial_desc) = feature.with_untracked(|feature| {
        (
            feature.name.clone(),
            feature.label().to_string(),
            feature.description.clone(),
        )
    });
    let anchor_id = Signal::derive(move || feature.read().dom_id());
    let stored_name = StoredValue::new(feature_name.clone());
    // A non-empty `name` means the row is bound to a registry entry; empty
    // `name` is a user-typed custom feat with only `label`. See on_input below.
    let is_readonly = !feature_name.is_empty();
    let (has_spells, has_empty_choices) = store
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
            (feature_data.spells.is_some(), has_empty)
        })
        .unwrap_or((false, false));
    let has_interactive_inputs = registry.with_features_index_untracked(|idx| {
        idx.get(feature_name.as_str())
            .is_some_and(|feat_def| feat_def.has_interactive_inputs())
    });

    let row_info = Memo::new(move |_| {
        let not_applied = !feature.applied().get();
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
    let field_count = Memo::new(move |_| {
        stored_name.with_value(|name| {
            store
                .features()
                .data()
                .read()
                .get(name)
                .map(|feature_data| feature_data.fields.len())
                .unwrap_or(0)
        })
    });
    let spell_link = has_spells.then(|| {
        let char_id = store.read_untracked().id;
        format!("/c/{char_id}/magic#{feature_name}")
    });

    // `None` means "no explicit user choice yet" — visibility falls back to
    // `:target` (CSS). Once the user clicks the toggle, we pin the row with
    // either `expanded` or `collapsed` class, with `:not(.collapsed)` in the
    // CSS rule keeping `:target` from resurrecting a collapsed row.
    let state: RwSignal<Option<bool>> = RwSignal::new(has_empty_choices.then_some(true));
    let feature_list_id = next_datalist_id();
    let toggle = move || {
        let currently_shown = state
            .get_untracked()
            .unwrap_or_else(|| location_hash_matches(&anchor_id.get_untracked()));
        state.set(Some(!currently_shown));
    };
    Some(view! {
        <div
            id=move || anchor_id.get()
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
                            {move || feature.read().label().to_string()}
                        </span>
                    })
                } else {
                    Either::Right(view! {
                        <SharedDatalist id=feature_list_id.clone() options=options />
                        <DatalistInput
                            value=initial_label
                            placeholder=move_tr!("feature-name")
                            class="entry-name"
                            list_id=feature_list_id.clone()
                            options=options
                            on_input=move |input, resolved| {
                                let key_for_apply = {
                                    let mut w = feature.write();
                                    if let Some(key) = resolved {
                                        // Resolved to a real feat: bind name to
                                        // the registry key and pull canonical
                                        // label/description from the index.
                                        w.name = key.clone();
                                        let (label, description) =
                                            registry.with_features_index(|index| {
                                                index.get(key.as_str())
                                                    .map(|feat| {
                                                        (
                                                            feat.label.clone(),
                                                            feat.description.clone(),
                                                        )
                                                    })
                                                    .unwrap_or_default()
                                            });
                                        w.label = label;
                                        w.description = description;
                                        Some(key)
                                    } else {
                                        // Free text: keep label only, drop any
                                        // prior registry binding so this row
                                        // stops being treated as an indexed feat.
                                        w.name.clear();
                                        w.set_label(input);
                                        w.description.clear();
                                        None
                                    }
                                };
                                if let Some(key) = key_for_apply {
                                    let is_non_interactive = registry.with_features_index_untracked(|index| {
                                        index.get(key.as_str())
                                            .is_some_and(|feat_def| !feat_def.has_interactive_inputs())
                                    });
                                    if is_non_interactive {
                                        let source = feature.with_untracked(|f| f.source.clone());
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
                            let (name, source, is_applied) = feature.with_untracked(
                                |feature| (feature.name.clone(), feature.source.clone(), feature.applied),
                            );
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
                    on:click=move |_| on_remove.run(())
                >
                    <Icon name="x" />
                </button>
            </div>
            {if is_readonly {
                Either::Left(view! {
                    <div class="entry-desc">
                        <Markdown text=Signal::derive(move || feature.read().description.clone()) />
                    </div>
                })
            } else {
                Either::Right(view! {
                    <textarea
                        class="entry-desc"
                        placeholder=move_tr!("description")
                        prop:value=initial_desc.clone()
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            feature.write().description = value;
                        }
                    />
                })
            }}
            {move || {
                let count = field_count.get();
                (count > 0).then(|| view! {
                    <div class="feature-fields" style="grid-column: 1 / -1">
                        {(0..count)
                            .map(|field_idx| view! {
                                <FeatureFieldRow feature_name=stored_name field_idx=field_idx />
                            })
                            .collect_view()}
                    </div>
                })
            }}
            {move || row_previews.with(|entries| (!entries.is_empty()).then(|| view! {
                <div class="entry-full-row feature-assignments">
                    {entries.iter().map(|entry| view! {
                        <span class="feature-assignment-entry">{entry.clone()}</span>
                    }).collect_view()}
                </div>
            }))}
        </div>
    })
}

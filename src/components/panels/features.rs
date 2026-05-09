use leptos::prelude::*;
use leptos_fluent::I18n;
use reactive_stores::{Field, Store, StoreFieldIterator};

use crate::{
    components::{
        add_feature_row::AddFeatureRow,
        apply::CaptureContext,
        build_hints::{
            BuildChoiceFillHint, BuildNeedsRebuildHint, BuildPendingApplyHint, BuildReplayHint,
        },
        datalist::DatalistOption,
        feature_row::FeatureRow,
    },
    hooks::use_query_signal,
    model::{
        Character, CharacterCoreStoreFields, CharacterStoreFields, Feature, FeatureCategory,
        FeaturesStoreFields, IdentitySlot,
    },
    rules::{RulesRegistry, WhenCondition, apply::apply_assignments_with_inputs},
};

#[component]
pub fn FeaturesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<I18n>();

    crate::hooks::use_scroll_to_hash();

    let (advanced, _) = use_query_signal::<bool>("advanced");
    let advanced = Signal::derive(move || advanced.get().unwrap_or(false));

    let features = store.core().features();

    let remove_feature = move |idx: usize| {
        let evict = {
            let list_signal = features.list();
            let mut list = list_signal.write();
            if idx >= list.len() {
                return;
            }
            let name = list.remove(idx).name;
            let still_applied = list
                .iter()
                .any(|feature| feature.name == name && feature.applied);
            (!still_applied).then_some(name)
        };
        if let Some(name) = evict {
            features.data().write().remove(&name);
        }
    };

    // Pipeline-order preview: shared `blank` accumulates across features so an
    // assign reading another feature's effect (e.g. STR.MOD after an ASI) sees
    // the up-to-date state, mirroring real apply order. Indexed by list
    // position so each row pulls its own preview row from the panel-level Memo.
    let assign_previews = Memo::new(move |_| {
        let mut blank = Character::default();
        let features_read = store.core().features().read();
        features_read
            .iter()
            .map(|feature| {
                let mut entries: Vec<String> = Vec::new();
                registry.with_feature(&feature.name, |feat_def| {
                    let mut ctx = CaptureContext {
                        character: &mut blank,
                        captured: Vec::new(),
                    };
                    if let Some(assignments) = feat_def.assign.as_ref() {
                        apply_assignments_with_inputs(
                            &mut ctx,
                            assignments,
                            WhenCondition::OnFeatureAdd,
                            &feature.inputs,
                            true,
                        );
                    }
                    for (attr, value) in ctx.captured {
                        entries.push(format!(
                            "{}: {}",
                            attr.display_name(i18n),
                            attr.format_value(value, i18n)
                        ));
                    }
                });
                entries
            })
            .collect::<Vec<_>>()
    });

    let feature_options = Memo::new(move |_| {
        let character = store.read();
        registry.with_features_index(|features_index| {
            features_index
                .values()
                .filter(|feat| feat.is_selectable() && feat.meets_prerequisites(&character))
                .map(|feat| {
                    let (label, description) = registry.feature_label_desc(&feat.name);
                    DatalistOption::with_signals(&*feat.name, label, description)
                })
                .collect::<Vec<_>>()
        })
    });

    view! {
        <BuildNeedsRebuildHint />
        <BuildPendingApplyHint />
        <BuildChoiceFillHint />
        <BuildReplayHint />
        <div class="entry-list">
            <AddFeatureRow options=feature_options />
            <For
                // Key by `(dom_id, idx)`: stable across normal edits (idx
                // doesn't shift) and forces a remount when a rebuild reorders
                // or shrinks the list — captures a fresh idx so `at_unkeyed`
                // can't read past the new len. Pure dom_id keying preserves
                // children across reorders, leaving the captured idx stale
                // and the inner `Field` reader panicking on `&inner[stale]`.
                each=move || {
                    let advanced_on = advanced.get();
                    features
                        .list()
                        .read()
                        .iter()
                        .enumerate()
                        .filter(|(_, feature)| {
                            advanced_on
                                || !matches!(
                                    feature.category,
                                    FeatureCategory::System(IdentitySlot::Class)
                                )
                        })
                        .rev()
                        .map(|(idx, feature)| (idx, feature.dom_id()))
                        .collect::<Vec<_>>()
                }
                key=|(idx, dom_id)| (*idx, dom_id.clone())
                let:row
            >
                {
                    let (idx, _dom_id) = row;
                    // Reactive group-head: row k is a group head if the next
                    // non-System feature in list order (i.e. the one rendered
                    // immediately above it after .rev()) has a different
                    // source — or doesn't exist. Recomputes on every list
                    // mutation so additions don't leave stale headers.
                    let header_label = Signal::derive(move || {
                        let list = features.list().read();
                        let feature = list.get(idx)?;
                        let is_group_head = list[idx + 1..]
                            .iter()
                            .find(|next| {
                                advanced.get()
                                    || !matches!(
                                        next.category,
                                        FeatureCategory::System(IdentitySlot::Class)
                                    )
                            })
                            .is_none_or(|next| next.source != feature.source);
                        is_group_head.then(|| registry.source_label(&feature.source, i18n))
                    });
                    let feature: Field<Feature> = features.list().at_unkeyed(idx).into();
                    let row_previews = Signal::derive(move || {
                        assign_previews
                            .with(|all| all.get(idx).cloned())
                            .unwrap_or_default()
                    });
                    // TODO: migrate to at_keyed so FeatureRow can detect zombie
                    // state internally — at_unkeyed Reader panics on stale idx.
                    let in_bounds = Signal::derive(move || idx < features.list().read().len());
                    view! {
                        {move || header_label.get().map(|label| view! {
                            <h3 class="features-group-header">{label}</h3>
                        })}
                        <Show when=move || in_bounds.get() fallback=|| ()>
                            <FeatureRow
                                feature=feature
                                options=feature_options
                                row_previews=row_previews
                                on_remove=Callback::new(move |()| remove_feature(idx))
                            />
                        </Show>
                    }
                }
            </For>
        </div>
    }
}

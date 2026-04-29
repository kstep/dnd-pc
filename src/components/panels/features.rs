use leptos::{prelude::*, reactive::wrappers::read::ArcSignal};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::{Field, Store, StoreFieldIterator};

use crate::{
    components::{
        apply::CaptureContext,
        build_hints::{
            BuildChoiceFillHint, BuildNeedsRebuildHint, BuildPendingApplyHint, BuildReplayHint,
        },
        datalist::DatalistOption,
        feature_row::FeatureRow,
    },
    model::{Character, CharacterStoreFields, Feature, FeatureSource, FeaturesStoreFields},
    rules::{RulesRegistry, WhenCondition, apply::apply_assignments_with_inputs},
};

#[component]
pub fn FeaturesPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<I18n>();

    crate::hooks::use_scroll_to_hash();

    let features = store.features();

    let add_feature = move |_| {
        let level = store.read_untracked().level();
        features.list().write().push(Feature {
            source: FeatureSource::User(level),
            ..Feature::default()
        });
    };

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
        let features_read = store.features().read();
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

    let prereq_prefix = move_tr!("prerequisites-label");
    let feature_options = Memo::new(move |_| {
        let character = store.read();
        registry.with_features_index(|features_index| {
            features_index
                .values()
                .filter(|feat| feat.is_selectable())
                .map(|feat| {
                    let (label, description) =
                        registry.features().label_desc(&*feat.name, &*feat.name);
                    let opt = DatalistOption::with_signals(&*feat.name, label, description);
                    if let Some(expr) = &feat.prerequisites
                        && !feat.meets_prerequisites(&character)
                    {
                        let expr_string = expr.to_string();
                        let reason = ArcSignal::derive(move || {
                            prereq_prefix.with(|prefix| format!("{prefix}: {expr_string}"))
                        });
                        opt.with_blocked_reason(reason)
                    } else {
                        opt
                    }
                })
                .collect::<Vec<_>>()
        })
    });

    view! {
        <BuildNeedsRebuildHint />
        <BuildPendingApplyHint />
        <BuildChoiceFillHint />
        <BuildReplayHint />
        <button class="btn-primary" on:click=add_feature>
            {move_tr!("btn-add-feature")}
        </button>
        <div class="entry-list">
            <For
                each=move || (0..features.list().read().len()).rev()
                key=|idx| *idx
                let:idx
            >
                {
                    // `header_label` reads features.list() reactively: after a
                    // remove the indices stay valid but the group-head boundary
                    // shifts, so the row at the new top has to re-evaluate.
                    let header_label = Signal::derive(move || {
                        let list = features.list().read();
                        let feature = list.get(idx)?;
                        let next_below = list.get(idx + 1);
                        let is_group_head = next_below
                            .is_none_or(|next| next.source != feature.source);
                        is_group_head.then(|| registry.source_label(&feature.source, i18n))
                    });
                    let feature: Field<Feature> = features.list().at_unkeyed(idx).into();
                    let row_previews = Signal::derive(move || {
                        assign_previews
                            .with(|all| all.get(idx).cloned())
                            .unwrap_or_default()
                    });
                    view! {
                        {move || header_label.get().map(|label| view! {
                            <h3 class="features-group-header">{label}</h3>
                        })}
                        <FeatureRow
                            feature=feature
                            options=feature_options
                            row_previews=row_previews
                            on_remove=Callback::new(move |()| remove_feature(idx))
                        />
                    }
                }
            </For>
        </div>
    }
}

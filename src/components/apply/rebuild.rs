use std::{collections::BTreeMap, sync::Arc};

use leptos::prelude::*;
use leptos_fluent::tr;
use leptos_router::{NavigateOptions, hooks::use_navigate};
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    components::{args_modal::ArgsModalCtx, toast::Toast},
    model::Character,
    rules::{
        ApplyInputs, RulesRegistry,
        apply::{
            DefinitionKind, FeatureKey, PendingInputs, RebuildError, RebuildOutcome,
            RebuildPreview, build_clean, prepare_rebuild,
        },
    },
};

/// Transactional rebuild: reconcile User-sourced features against identity
/// slots, collect pending user inputs. Try to commit silently by feeding the
/// guessed prefill through `build_clean` and checking whether the simulated
/// character matches `original` on the derived state. If so — swap and show
/// a toast. Otherwise — open the args modal with the partial prefill.
///
/// Aborts with a toast on multiclass prereq failure or missing
/// class/species/background definitions — the store is never written in that
/// case. Per-feature missing definitions never abort: User-source ones are
/// preserved as orphans, identity-source ones are dropped — both surfaced via
/// the post-rebuild "skipped/removed" toast.
#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "rebuild.total", skip_all)
)]
pub fn rebuild(store: Store<Character>, registry: RulesRegistry) {
    let RebuildPreview {
        original,
        pending,
        cascade_base,
        had_rejections,
    } = prepare_rebuild(store.get_untracked(), &registry);

    let open_build_tab = build_tab_action(original.id);

    let do_rebuild = {
        let original = original.clone();
        move |modal_inputs: Option<&ApplyInputs>| {
            let empty = ApplyInputs::default();
            let extra = modal_inputs.unwrap_or(&empty);
            match build_clean(&original, &registry, extra) {
                Ok(outcome) => commit_rebuild(store, outcome, false, &open_build_tab),
                Err(error) => show_rebuild_error(&error, &open_build_tab),
            }
        }
    };

    if pending.is_empty() {
        do_rebuild(None);
        return;
    }

    // Replaceable slots without a detected prefill (e.g. a fresh Versatile
    // that nobody has swapped yet) must ask the user — silent-commit would
    // lock them in as the original slot feat.
    let needs_replacement_choice = pending
        .iter()
        .any(|pi| pi.is_replaceable() && pi.prefilled_replacement.is_none());

    // Try silent commit only when pre-validation didn't reject any stored
    // inputs and every replaceable slot has a resolved prefill. Rejections
    // signal corruption (e.g. Expertise picks on no-longer-proficient
    // skills) — silently re-applying the same guessed prefill would just
    // replay that corruption without giving the user a chance to fix it.
    if !had_rejections && !needs_replacement_choice {
        let guessed = synthesize_apply_inputs(&pending);
        if let Ok(simulated) = build_clean(&original, &registry, &guessed)
            && simulated.character.eq_derived(&original)
        {
            log::info!("rebuild: silent-applied; derived state matches original");
            commit_rebuild(store, simulated, true, &open_build_tab);
            return;
        }
    }

    log::info!(
        "rebuild: {} — opening modal with partial prefill",
        if had_rejections {
            "rejected corrupted stored inputs"
        } else if needs_replacement_choice {
            "replaceable slot needs user pick"
        } else {
            "guess incomplete"
        }
    );
    // Cascade seed = identity + every effective-stored feat applied up to
    // the first emitted pending. Feats between emitted ones ride in
    // `pending` as `hidden=true` and are applied by the cascade Effect,
    // keeping `expr.analyze` pipeline-correct for every editable step.
    let base = Arc::new(cascade_base);
    let ctx = expect_context::<ArgsModalCtx>();
    ctx.open(pending, Some(base), move |inputs| do_rebuild(Some(&inputs)));
}

/// Convert `PendingInputs` prefill into `ApplyInputs` for a silent-commit
/// trial `build_clean`. `replacements` carries over `prefilled_replacement`
/// so subclass/Epic-Boon swaps recovered from `original.features` feed
/// back into `resolve_replacements` and the simulated character mirrors
/// the user's original choice.
fn synthesize_apply_inputs(pending: &[PendingInputs]) -> ApplyInputs {
    let replacements: BTreeMap<String, String> = pending
        .iter()
        .filter_map(|pending| {
            pending
                .prefilled_replacement
                .clone()
                .map(|replacement| (pending.feature_name.clone(), replacement))
        })
        .collect();
    ApplyInputs {
        feature_inputs: pending
            .iter()
            .map(|pending| {
                (
                    FeatureKey::new(&pending.feature_name, pending.source.clone()),
                    pending.prefill.clone(),
                )
            })
            .collect(),
        replacements,
    }
}

/// Build a callback that navigates to this character's Build tab. Captured at
/// `rebuild()` entry so `use_navigate()` runs in the active reactive context;
/// the resulting closure is cheap to clone and safe to invoke later from a
/// toast action button.
fn build_tab_action(character_id: Uuid) -> Callback<()> {
    let navigate = use_navigate();
    Callback::new(move |()| {
        navigate(
            &format!("/c/{character_id}/build"),
            NavigateOptions::default(),
        );
    })
}

fn commit_rebuild(
    store: Store<Character>,
    outcome: RebuildOutcome,
    silent: bool,
    open_build_tab: &Callback<()>,
) {
    let RebuildOutcome {
        character,
        skipped_features,
        removed_features,
    } = outcome;
    if !skipped_features.is_empty() {
        log::warn!("rebuild: preserved {skipped_features:?} as User orphans");
    }
    if !removed_features.is_empty() {
        log::warn!("rebuild: dropped obsolete identity features {removed_features:?}");
    }
    store.update(|character_store| {
        *character_store = character;
    });
    let issue_toast = match (skipped_features.len(), removed_features.len()) {
        (0, 0) => None,
        (skipped, 0) => Some(Toast::new(tr!(
            "toast-rebuild-skipped",
            { "count" => skipped }
        ))),
        (0, removed) => Some(Toast::new(tr!(
            "toast-rebuild-removed",
            { "count" => removed }
        ))),
        (skipped, removed) => Some(Toast::new(tr!(
            "toast-rebuild-skipped-and-removed",
            { "skipped" => skipped, "removed" => removed }
        ))),
    };
    // Drift toast implies completion — don't double up with the success toast.
    if let Some(toast) = issue_toast {
        toast
            .with_action(tr!("toast-rebuild-action-open-build"), *open_build_tab)
            .show();
    } else if silent {
        Toast::i18n_success("toast-rebuild-done").show();
    }
}

fn show_rebuild_error(error: &RebuildError, open_build_tab: &Callback<()>) {
    // Failed-class/species/background point at the character header (always
    // visible), not at the Build tab — no action button. Multiclass prereq
    // sends the user to Build tab features (Generation / ASI) — gets the
    // action.
    let (toast, with_action) = match error {
        RebuildError::MissingDefinition { kind, name } => {
            log::error!("rebuild aborted: missing {kind:?} definition '{name}'");
            let name = name.as_str();
            let toast = match kind {
                DefinitionKind::Class => {
                    Toast::error(tr!("toast-rebuild-failed-class", { "name" => name }))
                }
                DefinitionKind::Species => {
                    Toast::error(tr!("toast-rebuild-failed-species", { "name" => name }))
                }
                DefinitionKind::Background => {
                    Toast::error(tr!("toast-rebuild-failed-background", { "name" => name }))
                }
            };
            (toast, false)
        }
        RebuildError::MulticlassPrereq { class } => {
            log::error!("rebuild aborted: multiclass prereq for '{class}' never satisfied");
            let toast = Toast::error(tr!(
                "toast-rebuild-failed-multiclass",
                { "class" => class.as_str() }
            ));
            (toast, true)
        }
    };
    if with_action {
        toast
            .with_action(tr!("toast-rebuild-action-open-build"), *open_build_tab)
            .show();
    } else {
        toast.show();
    }
}

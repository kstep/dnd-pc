use std::{collections::BTreeMap, sync::Arc};

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::{args_modal::ArgsModalCtx, toast::Toast},
    model::Character,
    rules::{
        ApplyInputs, RulesRegistry,
        apply::{
            FeatureKey, PendingInputs, RebuildError, RebuildPreview, build_clean, prepare_rebuild,
        },
    },
};

/// Transactional rebuild: reconcile User-sourced features against identity
/// slots, collect pending user inputs. Try to commit silently by feeding the
/// guessed prefill through `build_clean` and checking whether the simulated
/// character matches `original` on the derived state. If so — swap and show
/// a toast. Otherwise — open the args modal with the partial prefill.
///
/// Aborts with a browser alert on multiclass prereq failure or missing
/// class/species/background definitions — the store is never written in that
/// case.
pub fn rebuild(store: Store<Character>, registry: RulesRegistry) {
    let RebuildPreview {
        original,
        pending,
        cascade_base,
        had_rejections,
    } = prepare_rebuild(store.get_untracked(), &registry);

    let do_rebuild = {
        let original = original.clone();
        move |modal_inputs: Option<&ApplyInputs>| {
            let empty = ApplyInputs::default();
            let extra = modal_inputs.unwrap_or(&empty);
            match build_clean(&original, &registry, extra) {
                Ok(clean) => {
                    store.update(|character| {
                        *character = clean;
                        registry.compute(character);
                    });
                }
                Err(err) => show_rebuild_error(&err),
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
            && simulated.eq_derived(&original)
        {
            log::info!("rebuild: silent-applied; derived state matches original");
            store.update(|character| {
                *character = simulated;
                registry.compute(character);
            });
            Toast::i18n_success("toast-rebuild-done").show();
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

fn show_rebuild_error(err: &RebuildError) {
    let message = match err {
        RebuildError::MissingDefinition { kind, name } => {
            format!("Rebuild failed: missing {kind} definition '{name}'.")
        }
        RebuildError::MulticlassPrereq { class } => {
            format!("Rebuild failed: multiclass prerequisites for '{class}' never satisfied.")
        }
    };
    log::error!("{message}");
    window().alert_with_message(&message).ok();
}

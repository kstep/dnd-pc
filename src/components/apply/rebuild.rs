use std::sync::Arc;

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::args_modal::ArgsModalCtx,
    model::Character,
    rules::{
        ApplyInputs, RulesRegistry,
        apply::{RebuildError, build_clean, prepare_rebuild},
    },
};

/// Transactional rebuild: reconcile User-sourced features against identity
/// slots, collect pending user inputs, open the args modal if any are
/// outstanding, then `build_clean` → swap store + compute.
///
/// Aborts with a browser alert on multiclass prereq failure or missing
/// class/species/background definitions — the store is never written in that
/// case.
pub fn rebuild(store: Store<Character>, registry: RulesRegistry) {
    let (original, pending_inputs) = prepare_rebuild(store.get_untracked(), &registry);

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

    if pending_inputs.is_empty() {
        do_rebuild(None);
    } else {
        // Seed cascade with a fresh identity-only character — matches what
        // `build_clean` starts from, so modal preview sees PROF=0 / default
        // abilities instead of the live sheet's residual state from prior
        // (possibly half-migrated) applies.
        let base = Arc::new(Character::from_identity(original.identity.clone()));
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open_with_base(pending_inputs, Some(base), move |inputs| {
            do_rebuild(Some(&inputs))
        });
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

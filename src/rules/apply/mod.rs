pub mod args_ctx;
mod collect;
mod compute;
pub mod context;
mod item_ctx;
mod pending;
mod primitives;
mod rebuild;
mod reconcile;
mod registry_ext;
mod solver;

pub use collect::{
    collect_background_features, collect_class_features, collect_pending_features,
    collect_species_features,
};
pub use compute::{assign, compute};
pub use context::{ApplyContext, apply_assignments_with_inputs};
pub use item_ctx::{ItemApplyCtx, assign_items};
pub use pending::{ApplyInputs, FeatureKey, PendingFeature, PendingInputs};
pub use primitives::{
    apply_new_features, build_cascade_base_before, dry_run_apply_feature, replay,
    resolve_replacements, restore_user_state,
};
pub use rebuild::{
    DefinitionKind, RebuildError, RebuildOutcome, RebuildPreview, build_clean, prepare_rebuild,
};
pub use reconcile::reconcile_user_feature_sources;
pub use registry_ext::apply_feature;

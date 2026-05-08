pub mod args_ctx;
mod caches;
mod collect;
mod compute;
pub mod context;
mod item_ctx;
mod pending;
pub mod plan;
mod primitives;
mod rebuild;
mod reconcile;
mod registry_ext;
mod solver;

pub use caches::{DefinitionCaches, IdentityChange};
pub use collect::{
    collect_background_features, collect_class_features, collect_pending_features,
    collect_species_features,
};
pub use compute::{assign, compute, compute_core};
pub use context::{ApplyContext, apply_assignments_with_inputs};
pub use item_ctx::{ItemApplyCtx, assign_items};
pub use pending::{ApplyInputs, FeatureKey, PendingFeature, PendingInputs, RecomputePending};
pub use primitives::{apply_pending, cascade, dry_run_apply_feature, restore_user_state};
pub use rebuild::{
    DefinitionKind, RebuildError, RebuildOutcome, RebuildPreview, build_clean, prepare_rebuild,
    rebuild_recompute,
};
pub use reconcile::reconcile_user_feature_sources;
pub use registry_ext::apply_feature;

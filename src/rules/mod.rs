pub mod apply;
pub use apply::{ApplyInputs, FeatureKey, PendingInputs, RecomputePending};
pub mod background;
mod cache;
pub mod class;
pub mod feature;
mod index;
mod labels;
pub mod locale;
mod preview;
mod registry;
mod resolve;
pub mod species;
pub mod spells;
mod summarizer;
pub mod utils;

pub use background::BackgroundDefinition;
pub use cache::DefinitionStore;
pub use class::{ClassDefinition, ClassLevelRules, SubclassDefinition};
pub use feature::{
    ActionDefinition, Assignment, ChoiceOption, ChoiceOptions, FeatureDefinition, FeaturesIndex,
    ReplaceWith, WhenCondition,
};
pub use index::{
    BackgroundIndexEntry, ClassIndexEntry, Index, IndexEntry, SpeciesIndexEntry, SpellIndexEntry,
};
pub use preview::{PreviewContext, eval_at_levels};
pub use registry::{FeaturesView, RulesRegistry, make_system_feature};
pub use species::SpeciesDefinition;
pub use spells::{
    CastTime, SpellCategory, SpellDefinition, SpellEntry, SpellMeta, SpellsDefinition, SpellsIndex,
    SpellsList,
};
pub use summarizer::{PoolKind, PoolSummarizer, PoolSummary, StickyGrant};
pub use utils::LevelRules;

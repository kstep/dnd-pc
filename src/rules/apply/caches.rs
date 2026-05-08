use std::collections::BTreeMap;
#[cfg(test)]
use std::sync::OnceLock;

use crate::rules::{
    background::BackgroundDefinition, class::ClassDefinition, species::SpeciesDefinition,
};

/// Identity-attribute mutation observed by [`ApplyContext::assign`]. The
/// outer apply layer drains these to update `character.applied` flags and
/// derive the corresponding feature follow-ups.
#[derive(Debug, Clone)]
pub enum IdentityChange {
    /// `CLASS.<class>.LEVEL` moved from `old` to `new` (`new > old`).
    ClassLevel { class: Box<str>, old: u32, new: u32 },
    /// `SUBCLASS.<name> = 1` was set on `class`.
    Subclass { class: Box<str>, name: Box<str> },
    /// `SPECIES.<name> = 1` was set.
    Species(Box<str>),
    /// `BACKGROUND.<name> = 1` was set.
    Background(Box<str>),
}

/// Borrowed bundle of class / species / background definition caches threaded
/// through the apply pipeline. Built by the caller from registry caches (see
/// [`RulesRegistry::with_definitions`]) or by tests from owned maps. `Copy` —
/// three references inside, passed by value to keep call-sites uncluttered.
#[derive(Clone, Copy)]
pub struct DefinitionCaches<'a> {
    pub classes: &'a BTreeMap<Box<str>, ClassDefinition>,
    pub species: &'a BTreeMap<Box<str>, SpeciesDefinition>,
    pub backgrounds: &'a BTreeMap<Box<str>, BackgroundDefinition>,
}

#[cfg(test)]
struct EmptyMaps {
    classes: BTreeMap<Box<str>, ClassDefinition>,
    species: BTreeMap<Box<str>, SpeciesDefinition>,
    backgrounds: BTreeMap<Box<str>, BackgroundDefinition>,
}

#[cfg(test)]
impl DefinitionCaches<'static> {
    /// Empty caches backed by `'static` empty maps. For tests that need a
    /// `DefinitionCaches` but never look anything up.
    pub fn empty() -> Self {
        static EMPTY: OnceLock<EmptyMaps> = OnceLock::new();
        let maps = EMPTY.get_or_init(|| EmptyMaps {
            classes: BTreeMap::new(),
            species: BTreeMap::new(),
            backgrounds: BTreeMap::new(),
        });
        Self {
            classes: &maps.classes,
            species: &maps.species,
            backgrounds: &maps.backgrounds,
        }
    }
}

use std::collections::BTreeMap;

use leptos::prelude::*;
use serde::{Deserialize, Deserializer};

use crate::{
    demap::{self, Named},
    model::EffectsIndex,
    rules::{feature::FeaturesIndex, spells::SpellsIndex},
    storage::load_active_packages,
    vecset::VecSet,
};

/// One entry of the package manifest (`public/rules/index.json`) — the ONLY
/// source of available packages; nothing about them is hardcoded at runtime.
#[derive(Clone, Deserialize)]
pub struct PackageManifestEntry {
    pub id: String,
    pub kind: PackageKind,
    pub name: String,
}

#[derive(Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    Base,
    Addon,
}

/// Test fixture set matching the shipped packages; runtime code must use
/// the fetched manifest instead.
#[cfg(any(test, feature = "testing"))]
pub fn test_packages() -> VecSet<String> {
    ["phb24", "efoa", "motm", "lorwyn", "grimhollow"]
        .map(str::to_string)
        .into_iter()
        .collect()
}

/// App-root context: the package set the registry currently resolves against.
/// Follows the open character; reference screens read the last active set.
#[derive(Clone, Copy)]
pub struct ActivePackages(pub RwSignal<VecSet<String>>);

impl ActivePackages {
    /// Restore the persisted set. Empty until the manifest resolves and the
    /// App-root effect fills it with every listed package.
    pub fn load() -> Self {
        let initial = load_active_packages().unwrap_or_default();
        Self(RwSignal::new(initial))
    }
}

/// Fold data fetched from a later package into an earlier accumulator;
/// overlay entries win on key collision.
pub trait PackageMerge {
    fn absorb(&mut self, overlay: Self);
}

impl<K: Ord, V> PackageMerge for BTreeMap<K, V> {
    fn absorb(&mut self, overlay: Self) {
        self.extend(overlay);
    }
}

impl PackageMerge for Vec<String> {
    fn absorb(&mut self, overlay: Self) {
        for name in overlay {
            if !self.contains(&name) {
                self.push(name);
            }
        }
    }
}

impl PackageMerge for FeaturesIndex {
    fn absorb(&mut self, overlay: Self) {
        self.0.absorb(overlay.0);
    }
}

impl PackageMerge for SpellsIndex {
    fn absorb(&mut self, overlay: Self) {
        self.0.absorb(overlay.0);
    }
}

impl PackageMerge for EffectsIndex {
    fn absorb(&mut self, overlay: Self) {
        self.0.absorb(overlay.0);
    }
}

/// Whole-file definitions index (classes/species/backgrounds of one or more
/// packages), keyed by name. JSON shape: array of named objects.
#[derive(Clone)]
pub struct DefsIndex<T>(pub BTreeMap<Box<str>, T>);

// Manual impl: derive(Default) would demand T: Default, which the
// definition types don't implement.
impl<T> Default for DefsIndex<T> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<'de, T: Named + Deserialize<'de>> Deserialize<'de> for DefsIndex<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        demap::named_map(deserializer).map(Self)
    }
}

impl<T> PackageMerge for DefsIndex<T> {
    fn absorb(&mut self, overlay: Self) {
        self.0.absorb(overlay.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absorb_later_package_wins() {
        let mut base: BTreeMap<Box<str>, i32> =
            BTreeMap::from([("Rage".into(), 1), ("Dash".into(), 1)]);
        base.absorb(BTreeMap::from([("Rage".into(), 2)]));
        assert_eq!(base.get("Rage"), Some(&2));
        assert_eq!(base.get("Dash"), Some(&1));
    }

    #[test]
    fn vec_absorb_unions_without_dupes() {
        let mut base = vec!["Fireball".to_string(), "Shield".to_string()];
        base.absorb(vec!["Shield".to_string(), "Fabricate".to_string()]);
        assert_eq!(base, ["Fireball", "Shield", "Fabricate"]);
    }

    #[test]
    fn defs_index_deserializes_array_and_merges() {
        #[derive(Clone, serde::Deserialize)]
        struct Dummy {
            name: Box<str>,
            value: i32,
        }
        impl Named for Dummy {
            fn name(&self) -> &str {
                &self.name
            }
        }
        let mut base: DefsIndex<Dummy> =
            serde_json::from_str(r#"[{"name": "Wizard", "value": 1}]"#).unwrap();
        let overlay: DefsIndex<Dummy> = serde_json::from_str(
            r#"[{"name": "Wizard", "value": 2}, {"name": "Artificer", "value": 3}]"#,
        )
        .unwrap();
        base.absorb(overlay);
        assert_eq!(base.0["Wizard"].value, 2);
        assert_eq!(base.0.len(), 2);
    }

    #[test]
    fn manifest_entry_deserializes() {
        let entry: PackageManifestEntry =
            serde_json::from_str(r#"{"id": "phb24", "kind": "base", "name": "PHB"}"#).unwrap();
        assert!(entry.kind == PackageKind::Base);
        assert_eq!(entry.id, "phb24");
    }
}

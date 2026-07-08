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

/// Merge `overlay` (fetched from `package`) into self; overlay wins by name.
/// Definition containers stamp `entry.package` while merging; locale maps and
/// name lists ignore `package`.
pub trait PackageMerge {
    fn absorb(&mut self, overlay: Self, package: &str);
}

/// Definitions that record their source package (stamped during merge).
pub trait HasPackage {
    fn set_package(&mut self, package: &str);
}

impl<K: Ord, V> PackageMerge for BTreeMap<K, V> {
    fn absorb(&mut self, overlay: Self, _package: &str) {
        self.extend(overlay);
    }
}

impl PackageMerge for Vec<String> {
    fn absorb(&mut self, overlay: Self, _package: &str) {
        for name in overlay {
            if !self.contains(&name) {
                self.push(name);
            }
        }
    }
}

/// Stamp every `overlay` entry with `package`, then merge overlay-wins into
/// `target`. Shared body for every `PackageMerge` impl keyed by name.
fn stamp_absorb<T: HasPackage>(
    target: &mut BTreeMap<Box<str>, T>,
    mut overlay: BTreeMap<Box<str>, T>,
    package: &str,
) {
    for def in overlay.values_mut() {
        def.set_package(package);
    }
    target.extend(overlay);
}

impl PackageMerge for FeaturesIndex {
    fn absorb(&mut self, overlay: Self, package: &str) {
        stamp_absorb(&mut self.0, overlay.0, package);
    }
}

impl PackageMerge for SpellsIndex {
    fn absorb(&mut self, overlay: Self, package: &str) {
        stamp_absorb(&mut self.0, overlay.0, package);
    }
}

impl PackageMerge for EffectsIndex {
    fn absorb(&mut self, overlay: Self, package: &str) {
        stamp_absorb(&mut self.0, overlay.0, package);
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

impl<T: HasPackage> PackageMerge for DefsIndex<T> {
    fn absorb(&mut self, overlay: Self, package: &str) {
        stamp_absorb(&mut self.0, overlay.0, package);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absorb_later_package_wins() {
        let mut base: BTreeMap<Box<str>, i32> =
            BTreeMap::from([("Rage".into(), 1), ("Dash".into(), 1)]);
        base.absorb(BTreeMap::from([("Rage".into(), 2)]), "");
        assert_eq!(base.get("Rage"), Some(&2));
        assert_eq!(base.get("Dash"), Some(&1));
    }

    #[test]
    fn vec_absorb_unions_without_dupes() {
        let mut base = vec!["Fireball".to_string(), "Shield".to_string()];
        base.absorb(vec!["Shield".to_string(), "Fabricate".to_string()], "");
        assert_eq!(base, ["Fireball", "Shield", "Fabricate"]);
    }

    #[test]
    fn defs_index_deserializes_array_and_merges() {
        #[derive(Clone, Default, serde::Deserialize)]
        struct Dummy {
            #[serde(skip)]
            package: Box<str>,
            name: Box<str>,
            value: i32,
        }
        impl Named for Dummy {
            fn name(&self) -> &str {
                &self.name
            }
        }
        impl HasPackage for Dummy {
            fn set_package(&mut self, package: &str) {
                self.package = package.into();
            }
        }
        let mut base: DefsIndex<Dummy> =
            serde_json::from_str(r#"[{"name": "Wizard", "value": 1}]"#).unwrap();
        let overlay: DefsIndex<Dummy> = serde_json::from_str(
            r#"[{"name": "Wizard", "value": 2}, {"name": "Artificer", "value": 3}]"#,
        )
        .unwrap();
        base.absorb(overlay, "pkg");
        assert_eq!(base.0["Wizard"].value, 2);
        assert_eq!(base.0.len(), 2);
        assert_eq!(&*base.0["Artificer"].package, "pkg");
    }

    #[test]
    fn absorb_stamps_overlay_entries() {
        let mut base: FeaturesIndex = serde_json::from_str(r#"[{"name": "Rage"}]"#).unwrap();
        base.absorb(
            serde_json::from_str(r#"[{"name": "Flight"}]"#).unwrap(),
            "motm",
        );
        assert_eq!(&*base.0["Flight"].package, "motm");
        assert_eq!(&*base.0["Rage"].package, "");
    }

    #[test]
    fn manifest_entry_deserializes() {
        let entry: PackageManifestEntry =
            serde_json::from_str(r#"{"id": "phb24", "kind": "base", "name": "PHB"}"#).unwrap();
        assert!(entry.kind == PackageKind::Base);
        assert_eq!(entry.id, "phb24");
    }
}

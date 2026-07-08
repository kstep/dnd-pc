use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use dnd_pc::rules::{
    BackgroundDefinition, ClassDefinition, DefsIndex, FeaturesIndex, PackageMerge,
    SpeciesDefinition, SpellsIndex,
    locale::{LocaleMap, SpellsLocaleMap},
};
use serde::de::DeserializeOwned;

fn rules_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("public/rules")
        .leak()
}

/// Package data dirs (every subdirectory of public/rules).
fn package_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(rules_dir())
        .expect("read public/rules")
        .filter_map(|entry| entry.ok().map(|dir_entry| dir_entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "no packages under public/rules");
    dirs
}

fn parse_json<T: DeserializeOwned>(path: &Path) -> T {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn parse_if_exists<T: DeserializeOwned>(path: &Path) -> Option<T> {
    path.is_file().then(|| parse_json(path))
}

const LOCALES: &[&str] = &["en", "ru"];

// --- Manifest ---

#[test]
fn manifest_matches_package_dirs() {
    let manifest: Vec<serde_json::Value> = parse_json(&rules_dir().join("index.json"));
    for entry in &manifest {
        let id = entry["id"].as_str().expect("manifest id");
        assert!(
            rules_dir().join(id).is_dir(),
            "manifest package {id} has no dir"
        );
        assert!(
            entry["kind"] == "base" || entry["kind"] == "addon",
            "package {id} has invalid kind"
        );
        assert!(entry["name"].is_string(), "package {id} has no name");
    }
    assert_eq!(
        manifest
            .iter()
            .filter(|entry| entry["kind"] == "base")
            .count(),
        1,
        "exactly one base package"
    );
    assert_eq!(
        manifest.len(),
        package_dirs().len(),
        "every package dir must be listed in the manifest"
    );
}

// --- Structural data (per package) ---

fn validate_defs<T: DeserializeOwned + dnd_pc::demap::Named>(file: &str) {
    for pkg in package_dirs() {
        let Some(index) = parse_if_exists::<DefsIndex<T>>(&pkg.join("data").join(file)) else {
            continue;
        };
        for (name, def) in &index.0 {
            assert_eq!(
                def.name(),
                &**name,
                "name mismatch in {}/{file}",
                pkg.display()
            );
        }
    }
}

#[test]
fn data_classes_valid() {
    validate_defs::<ClassDefinition>("classes.json");
}

#[test]
fn data_species_valid() {
    validate_defs::<SpeciesDefinition>("species.json");
}

#[test]
fn data_backgrounds_valid() {
    validate_defs::<BackgroundDefinition>("backgrounds.json");
}

#[test]
fn data_features_valid() {
    for pkg in package_dirs() {
        let _: Option<FeaturesIndex> = parse_if_exists(&pkg.join("data/features.json"));
    }
}

#[test]
fn data_spells_index_valid() {
    for pkg in package_dirs() {
        let _: Option<SpellsIndex> = parse_if_exists(&pkg.join("data/spells.json"));
    }
}

#[test]
fn data_effects_valid() {
    for pkg in package_dirs() {
        let _: Option<Vec<serde_json::Value>> = parse_if_exists(&pkg.join("data/effects.json"));
    }
}

#[test]
fn no_cross_package_name_collisions() {
    // The initial split must be disjoint; overrides come later with dnd2014.
    for file in [
        "classes.json",
        "species.json",
        "backgrounds.json",
        "features.json",
        "spells.json",
    ] {
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for pkg in package_dirs() {
            let Some(items) =
                parse_if_exists::<Vec<serde_json::Value>>(&pkg.join("data").join(file))
            else {
                continue;
            };
            let pkg_name = pkg.file_name().unwrap().to_string_lossy().to_string();
            for item in items {
                let name = item["name"].as_str().expect("name").to_string();
                if let Some(other) = seen.insert(name.clone(), pkg_name.clone()) {
                    panic!("{file}: {name:?} defined in both {other} and {pkg_name}");
                }
            }
        }
    }
}

#[test]
fn spell_lists_resolve_into_merged_index() {
    let mut merged = SpellsIndex::default();
    for pkg in package_dirs() {
        if let Some(part) = parse_if_exists::<SpellsIndex>(&pkg.join("data/spells.json")) {
            let pkg_id = pkg.file_name().unwrap().to_string_lossy().to_string();
            merged.absorb(part, &pkg_id);
        }
    }
    let mut lists_seen = 0;
    for pkg in package_dirs() {
        let Ok(entries) = fs::read_dir(pkg.join("data/spells")) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("dir entry").path();
            let names: Vec<String> = parse_json(&path);
            assert!(!names.is_empty(), "{} is empty", path.display());
            lists_seen += 1;
            for spell_name in &names {
                assert!(
                    merged.0.contains_key(spell_name.as_str()),
                    "{} references unknown spell {spell_name:?}",
                    path.display()
                );
            }
        }
    }
    assert!(lists_seen >= 9, "expected at least the 9 caster lists");
}

// --- Locale overlays: deserialization ---

#[test]
fn locale_overlays_valid() {
    for pkg in package_dirs() {
        for locale in LOCALES {
            for file in [
                "classes.json",
                "species.json",
                "backgrounds.json",
                "features.json",
            ] {
                let _: Option<LocaleMap> = parse_if_exists(&pkg.join(locale).join(file));
            }
            let _: Option<SpellsLocaleMap> = parse_if_exists(&pkg.join(locale).join("spells.json"));
            let _: Option<LocaleMap> = parse_if_exists(&pkg.join(locale).join("effects.json"));
        }
    }
}

// --- Locale completeness: all translations present and non-empty ---

/// Every entity in a package's data file must have a same-package locale
/// entry keyed by its name, with a description (and a label outside `en`,
/// where the bare name is the natural fallback).
fn check_locale_complete(file: &str) {
    for pkg in package_dirs() {
        let Some(items) = parse_if_exists::<Vec<serde_json::Value>>(&pkg.join("data").join(file))
        else {
            continue;
        };
        let names: Vec<String> = items
            .iter()
            .map(|item| item["name"].as_str().expect("name").to_string())
            .collect();
        for locale in LOCALES {
            let check_label = locale != &"en";
            let map: LocaleMap = parse_json(&pkg.join(locale).join(file));
            for name in &names {
                let entry = map.get(name.as_str()).unwrap_or_else(|| {
                    panic!(
                        "[{locale}] {}/{file}: missing entry for {name}",
                        pkg.display()
                    )
                });
                assert!(
                    entry.description.as_ref().is_some_and(|d| !d.is_empty()),
                    "[{locale}] {}/{file}: '{name}' has no description",
                    pkg.display()
                );
                if check_label {
                    assert!(
                        entry.label.as_ref().is_some_and(|l| !l.is_empty()),
                        "[{locale}] {}/{file}: '{name}' has no label",
                        pkg.display()
                    );
                }
            }
        }
    }
}

#[test]
fn locale_classes_complete() {
    check_locale_complete("classes.json");

    // Subclasses: "{Class}.subclass.{Sub}" keys in the same file.
    for pkg in package_dirs() {
        let Some(index) =
            parse_if_exists::<DefsIndex<ClassDefinition>>(&pkg.join("data/classes.json"))
        else {
            continue;
        };
        for locale in LOCALES {
            let check_label = locale != &"en";
            let map: LocaleMap = parse_json(&pkg.join(locale).join("classes.json"));
            for (class_name, class_def) in &index.0 {
                for subclass_name in class_def.subclasses.keys() {
                    let key = format!("{class_name}.subclass.{subclass_name}");
                    let entry = map.get(key.as_str()).unwrap_or_else(|| {
                        panic!("[{locale}] {}: missing entry for {key}", pkg.display())
                    });
                    if check_label {
                        assert!(
                            entry.label.as_ref().is_some_and(|l| !l.is_empty()),
                            "[{locale}] {}: '{key}' has no label",
                            pkg.display()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn locale_species_complete() {
    check_locale_complete("species.json");
}

#[test]
fn locale_backgrounds_complete() {
    check_locale_complete("backgrounds.json");
}

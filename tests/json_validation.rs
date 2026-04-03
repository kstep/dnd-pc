use std::{fs, path::Path};

use dnd_pc::rules::{
    BackgroundDefinition, ClassDefinition, FeaturesIndex, Index, SpeciesDefinition, SpellMap,
    locale::{IndexLocaleMap, LocaleMap, SpellLocaleMap},
};
use serde::de::DeserializeOwned;

fn public_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("public").leak()
}

fn parse_json<T: DeserializeOwned>(path: &Path) -> T {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

const LOCALES: &[&str] = &["en", "ru"];

// --- Structural data (public/data/) ---

#[test]
fn data_index_valid() {
    let _: Index = parse_json(&public_dir().join("data/index.json"));
}

#[test]
fn data_classes_valid() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));
    for (name, entry) in &index.classes {
        let path = public.join(format!("data/{}", entry.url));
        let def: ClassDefinition = parse_json(&path);
        assert_eq!(&*def.name, &**name, "class name mismatch in {}", entry.url);
    }
}

#[test]
fn data_species_valid() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));
    for (name, entry) in &index.species {
        let path = public.join(format!("data/{}", entry.url));
        let def: SpeciesDefinition = parse_json(&path);
        assert_eq!(
            &*def.name, &**name,
            "species name mismatch in {}",
            entry.url
        );
    }
}

#[test]
fn data_backgrounds_valid() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));
    for (name, entry) in &index.backgrounds {
        let path = public.join(format!("data/{}", entry.url));
        let def: BackgroundDefinition = parse_json(&path);
        assert_eq!(
            &*def.name, &**name,
            "background name mismatch in {}",
            entry.url
        );
    }
}

#[test]
fn data_spell_lists_valid() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));
    for entry in index.spells.values() {
        let path = public.join(format!("data/{}", entry.url));
        let _: SpellMap = parse_json(&path);
    }
}

#[test]
fn data_features_valid() {
    let _: FeaturesIndex = parse_json(&public_dir().join("data/features.json"));
}

#[test]
fn data_effects_valid() {
    let _: Vec<serde_json::Value> = parse_json(&public_dir().join("data/effects.json"));
}

// --- Locale overlays: deserialization ---

#[test]
fn locale_index_valid() {
    let public = public_dir();
    for locale in LOCALES {
        let path = public.join(format!("{locale}/index.json"));
        let map: IndexLocaleMap = parse_json(&path);
        assert!(!map.is_empty(), "[{locale}] index overlay is empty");
    }
}

#[test]
fn locale_classes_valid() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));
    for locale in LOCALES {
        for entry in index.classes.values() {
            let path = public.join(format!("{locale}/{}", entry.url));
            let _: LocaleMap = parse_json(&path);
        }
    }
}

#[test]
fn locale_species_valid() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));
    for locale in LOCALES {
        for entry in index.species.values() {
            let path = public.join(format!("{locale}/{}", entry.url));
            let _: LocaleMap = parse_json(&path);
        }
    }
}

#[test]
fn locale_backgrounds_valid() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));
    for locale in LOCALES {
        for entry in index.backgrounds.values() {
            let path = public.join(format!("{locale}/{}", entry.url));
            let _: LocaleMap = parse_json(&path);
        }
    }
}

#[test]
fn locale_spell_lists_valid() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));
    for locale in LOCALES {
        for entry in index.spells.values() {
            let path = public.join(format!("{locale}/{}", entry.url));
            let _: SpellLocaleMap = parse_json(&path);
        }
    }
}

#[test]
fn locale_features_valid() {
    let public = public_dir();
    for locale in LOCALES {
        let path = public.join(format!("{locale}/features.json"));
        let _: LocaleMap = parse_json(&path);
    }
}

#[test]
fn locale_effects_valid() {
    let public = public_dir();
    for locale in LOCALES {
        let path = public.join(format!("{locale}/effects.json"));
        let _: IndexLocaleMap = parse_json(&path);
    }
}

// --- Locale completeness: all translations present and non-empty ---

#[test]
fn locale_index_complete() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));

    for locale in LOCALES {
        let map: IndexLocaleMap = parse_json(&public.join(format!("{locale}/index.json")));

        let check_label = locale != &"en";

        for name in index.classes.keys() {
            let key = format!("class.{name}");
            let entry = map
                .get(key.as_str())
                .unwrap_or_else(|| panic!("[{locale}] missing index entry for {key}"));
            assert!(
                entry.description.as_ref().is_some_and(|d| !d.is_empty()),
                "[{locale}] class '{name}' has no description in index"
            );
            if check_label {
                assert!(
                    entry.label.as_ref().is_some_and(|l| !l.is_empty()),
                    "[{locale}] class '{name}' has no label in index"
                );
            }
        }

        for name in index.species.keys() {
            let key = format!("species.{name}");
            let entry = map
                .get(key.as_str())
                .unwrap_or_else(|| panic!("[{locale}] missing index entry for {key}"));
            assert!(
                entry.description.as_ref().is_some_and(|d| !d.is_empty()),
                "[{locale}] species '{name}' has no description in index"
            );
            if check_label {
                assert!(
                    entry.label.as_ref().is_some_and(|l| !l.is_empty()),
                    "[{locale}] species '{name}' has no label in index"
                );
            }
        }

        for name in index.backgrounds.keys() {
            let key = format!("background.{name}");
            let entry = map
                .get(key.as_str())
                .unwrap_or_else(|| panic!("[{locale}] missing index entry for {key}"));
            assert!(
                entry.description.as_ref().is_some_and(|d| !d.is_empty()),
                "[{locale}] background '{name}' has no description in index"
            );
            if check_label {
                assert!(
                    entry.label.as_ref().is_some_and(|l| !l.is_empty()),
                    "[{locale}] background '{name}' has no label in index"
                );
            }
        }
    }
}

#[test]
fn locale_classes_complete() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));

    for locale in LOCALES {
        let check_label = locale != &"en";

        for (class_name, entry) in &index.classes {
            let data_def: ClassDefinition = parse_json(&public.join(format!("data/{}", entry.url)));
            let locale_map: LocaleMap = parse_json(&public.join(format!("{locale}/{}", entry.url)));

            let root = locale_map
                .get("")
                .unwrap_or_else(|| panic!("[{locale}] class '{class_name}' missing root entry"));
            assert!(
                root.description.as_ref().is_some_and(|d| !d.is_empty()),
                "[{locale}] class '{class_name}' missing root description"
            );
            if check_label {
                assert!(
                    root.label.as_ref().is_some_and(|l| !l.is_empty()),
                    "[{locale}] class '{class_name}' missing root label"
                );
            }

            for subclass_name in data_def.subclasses.keys() {
                let key = format!("subclass.{subclass_name}");
                let sub = locale_map.get(key.as_str()).unwrap_or_else(|| {
                    panic!("[{locale}] class '{class_name}' missing entry for '{key}'")
                });
                assert!(
                    sub.description.as_ref().is_some_and(|d| !d.is_empty()),
                    "[{locale}] class '{class_name}' subclass '{subclass_name}' missing description"
                );
                if check_label {
                    assert!(
                        sub.label.as_ref().is_some_and(|l| !l.is_empty()),
                        "[{locale}] class '{class_name}' subclass '{subclass_name}' missing label"
                    );
                }
            }
        }
    }
}

#[test]
fn locale_species_complete() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));

    for locale in LOCALES {
        let check_label = locale != &"en";

        for (species_name, entry) in &index.species {
            let locale_map: LocaleMap = parse_json(&public.join(format!("{locale}/{}", entry.url)));
            let root = locale_map.get("").unwrap_or_else(|| {
                panic!("[{locale}] species '{species_name}' missing root entry")
            });
            assert!(
                root.description.as_ref().is_some_and(|d| !d.is_empty()),
                "[{locale}] species '{species_name}' missing root description"
            );
            if check_label {
                assert!(
                    root.label.as_ref().is_some_and(|l| !l.is_empty()),
                    "[{locale}] species '{species_name}' missing root label"
                );
            }
        }
    }
}

#[test]
fn locale_backgrounds_complete() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));

    for locale in LOCALES {
        let check_label = locale != &"en";

        for (bg_name, entry) in &index.backgrounds {
            let locale_map: LocaleMap = parse_json(&public.join(format!("{locale}/{}", entry.url)));
            let root = locale_map
                .get("")
                .unwrap_or_else(|| panic!("[{locale}] background '{bg_name}' missing root entry"));
            assert!(
                root.description.as_ref().is_some_and(|d| !d.is_empty()),
                "[{locale}] background '{bg_name}' missing root description"
            );
            if check_label {
                assert!(
                    root.label.as_ref().is_some_and(|l| !l.is_empty()),
                    "[{locale}] background '{bg_name}' missing root label"
                );
            }
        }
    }
}

#[test]
fn locale_features_complete() {
    let public = public_dir();
    let features: FeaturesIndex = parse_json(&public.join("data/features.json"));

    for locale in LOCALES {
        let check_label = locale != &"en";
        let locale_map: LocaleMap = parse_json(&public.join(format!("{locale}/features.json")));

        let mut missing = Vec::new();
        let mut missing_labels = Vec::new();
        for name in features.0.keys() {
            match locale_map.get(name.as_ref()) {
                None => missing.push(name.as_ref()),
                Some(entry) if check_label => {
                    if !entry.label.as_ref().is_some_and(|l| !l.is_empty()) {
                        missing_labels.push(name.as_ref());
                    }
                }
                _ => {}
            }
        }
        assert!(
            missing.is_empty(),
            "[{locale}] features.json missing {} translations: {}",
            missing.len(),
            missing.join(", ")
        );
        assert!(
            missing_labels.is_empty(),
            "[{locale}] features.json missing {} labels: {}",
            missing_labels.len(),
            missing_labels.join(", ")
        );
    }
}

#[test]
fn locale_spells_complete() {
    let public = public_dir();
    let index: Index = parse_json(&public.join("data/index.json"));

    for locale in LOCALES {
        for (list_name, entry) in &index.spells {
            let data_spells: SpellMap = parse_json(&public.join(format!("data/{}", entry.url)));
            let locale_map: SpellLocaleMap =
                parse_json(&public.join(format!("{locale}/{}", entry.url)));

            let mut missing = Vec::new();
            for name in data_spells.0.keys() {
                if !locale_map.contains_key(name) {
                    missing.push(name.as_ref());
                }
            }
            assert!(
                missing.is_empty(),
                "[{locale}] spell list '{list_name}' missing {} translations: {}",
                missing.len(),
                missing.join(", ")
            );
        }
    }
}

#[test]
fn locale_effects_complete() {
    let public = public_dir();
    let data: Vec<serde_json::Value> = parse_json(&public.join("data/effects.json"));
    let effect_names: Vec<&str> = data
        .iter()
        .filter_map(|v| v.get("name")?.as_str())
        .collect();

    for locale in LOCALES {
        let locale_map: IndexLocaleMap = parse_json(&public.join(format!("{locale}/effects.json")));

        let mut missing = Vec::new();
        for name in &effect_names {
            if !locale_map.contains_key(*name) {
                missing.push(*name);
            }
        }
        assert!(
            missing.is_empty(),
            "[{locale}] effects.json missing {} translations: {}",
            missing.len(),
            missing.join(", ")
        );
    }
}

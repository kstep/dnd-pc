//! Native validation that the project's Fluent locale files load cleanly:
//! syntax-correct and free of duplicate message ids. Mirrors the set
//! `fluent-templates::static_loader!` pulls in from `src/lib.rs`.

use std::{fs, path::PathBuf};

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::{LanguageIdentifier, langid};

fn locales_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn validate_locale(lang: &str, langid: &LanguageIdentifier) {
    let path = locales_dir().join(lang).join("main.ftl");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

    // FluentResource::try_new parses the source and reports syntax errors.
    let resource = FluentResource::try_new(source)
        .unwrap_or_else(|(_, errors)| panic!("syntax errors in {}: {errors:?}", path.display()));

    // FluentBundle::add_resource catches semantic loader errors —
    // duplicate message ids, term/message conflicts, etc.
    let mut bundle: FluentBundle<FluentResource> = FluentBundle::new(vec![langid.clone()]);
    if let Err(errors) = bundle.add_resource(resource) {
        panic!("loader errors in {}: {errors:?}", path.display());
    }
}

#[test]
fn locale_en_valid() {
    validate_locale("en", &langid!("en"));
}

#[test]
fn locale_ru_valid() {
    validate_locale("ru", &langid!("ru"));
}

use std::{ffi::OsStr, fs, path::Path};

/// Walk `dir` recursively, collecting (relative-key, absolute-path) pairs
/// for every `*.json` file except `manifest.json`.
fn collect_json_files(base: &Path, dir: &Path, entries: &mut Vec<(String, String)>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(base, &path, entries);
        } else if path.extension() == Some(OsStr::new("json"))
            && path.file_name() != Some(OsStr::new("manifest.json"))
        {
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let abs = path.to_string_lossy().into_owned();
            entries.push((rel, abs));
        }
    }
}

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let public_dir = Path::new(&manifest_dir).join("public");

    // Rebuild whenever any public JSON file changes.
    println!("cargo:rerun-if-changed=public/");

    let mut entries: Vec<(String, String)> = Vec::new();
    collect_json_files(&public_dir, &public_dir, &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut code = "fn public_json_files() -> std::collections::HashMap<&'static str, &'static str> {\n    let mut m = std::collections::HashMap::new();\n".to_string();
    for (rel, abs) in &entries {
        code.push_str(&format!("    m.insert({rel:?}, include_str!({abs:?}));\n"));
    }
    code.push_str("    m\n}\n");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out_dir).join("public_json_files.rs"), code).unwrap();
}

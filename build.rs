use std::{env, path::PathBuf, process::Command};

fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let date = Command::new("git")
        .args(["log", "-1", "--format=%cI"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    // Custom profile name lives in the OUT_DIR path:
    //   target/<profile>/build/<hash>/out
    //   target/wasm32-unknown-unknown/<profile>/build/<hash>/out
    // Cargo's own `PROFILE` env var collapses every custom profile down to
    // its inherit root ("release"), so parsing the path is the only way to
    // preserve names like "perf".
    let profile = env::var("OUT_DIR")
        .ok()
        .and_then(|out| {
            PathBuf::from(out)
                .ancestors()
                .nth(3)
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=BUILD_DATE={date}");
    println!("cargo:rustc-env=BUILD_PROFILE={profile}");
    println!("cargo:rerun-if-env-changed=PROXY_URL");
    // Rebuild when HEAD moves (new commit, checkout, rebase, amend) so
    // BUILD_COMMIT/BUILD_DATE stay in sync. Resolve through `git rev-parse`
    // to get the real path — in a worktree `.git` is a pointer file and
    // `.git/HEAD` doesn't exist literally.
    for path in ["HEAD", "logs/HEAD"] {
        if let Some(resolved) = Command::new("git")
            .args(["rev-parse", "--git-path", path])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|p| !p.is_empty())
        {
            println!("cargo:rerun-if-changed={resolved}");
        }
    }
}

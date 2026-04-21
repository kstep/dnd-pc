use std::process::Command;

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

    println!("cargo:rustc-env=BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=BUILD_DATE={date}");
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

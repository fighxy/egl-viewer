use std::process::Command;

fn main() {
    // The short commit for the About dialog; absent outside a git checkout.
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|commit| commit.trim().to_owned())
        .filter(|commit| !commit.is_empty());
    if let Some(commit) = commit {
        println!("cargo:rustc-env=EGL_COMMIT={commit}");
    }
    // A path that does not exist would rerun this script on every build.
    for path in ["../.git/HEAD", "../.git/refs/heads"] {
        if std::path::Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    tauri_build::build();
}

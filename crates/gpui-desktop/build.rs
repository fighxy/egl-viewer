use std::process::Command;

fn main() {
    // Short commit for About; absent outside a git checkout (same as src-tauri).
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
    // The whole shell is one GPUI element tree built and painted on the main thread. Debug
    // builds keep every builder temporary on the stack, and Windows gives the main thread
    // 1 MiB: opening a library overflowed it (0xC00000FD). 8 MiB, the Linux default (D-141).
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
    for path in ["../../.git/HEAD", "../../.git/refs/heads"] {
        if std::path::Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

use std::process::Command;

fn main() {
    // Short commit for About; absent outside a git checkout.
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
    // The exe icon (D-144): GPUI's Windows backend loads icon resource 1 for every window,
    // so without it the title bar and the taskbar show the generic application icon.
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=app.rc");
        println!("cargo:rerun-if-changed=assets/icon.ico");
        println!("cargo:rerun-if-changed=assets/cursors/grab.cur");
        println!("cargo:rerun-if-changed=assets/cursors/grabbing.cur");
        embed_resource::compile("app.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("compile the exe resources (app.rc)");
    }
    for path in ["../../.git/HEAD", "../../.git/refs/heads"] {
        if std::path::Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

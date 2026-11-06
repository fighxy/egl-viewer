//! Link flags for the shared library the phones load (D-115).

fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if os == "android" || os == "linux" {
        // Without a SONAME, a library linked against `libegl_library.so` (Android's
        // `libegl_jni.so`) records the build machine's absolute path to it as DT_NEEDED,
        // and the phone cannot load it.
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libegl_library.so");
    }
    if os == "android" {
        // Android 15+ devices with 16 KB pages refuse libraries aligned to 4 KB.
        println!("cargo:rustc-cdylib-link-arg=-Wl,-z,max-page-size=16384");
    }
}

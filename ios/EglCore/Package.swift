// swift-tools-version:5.9
// EglCore: the egl-viewer Rust core (crates/library-ffi over crates/app-service) for Swift,
// with typed models and errors (D-092). The app links `libegl_library` (a static library for
// iOS, a dylib for the host tests); the search path comes from the build (`-Xlinker -L…`).
import PackageDescription

let package = Package(
    name: "EglCore",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [
        .library(name: "EglCore", targets: ["EglCore"]),
    ],
    targets: [
        .systemLibrary(name: "CEglLibrary", path: "Sources/CEglLibrary"),
        .target(name: "EglCore", dependencies: ["CEglLibrary"]),
        .testTarget(name: "EglCoreTests", dependencies: ["EglCore"]),
    ]
)

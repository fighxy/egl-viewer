plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "app.eglviewer.core"
    compileSdk = 35

    // Enable CMake / NDK when the Rust cdylib is present (or -Pegl.ffi=true).
    // Mobile CI builds libegl_library.so with cargo-ndk before assemble (D-101).
    // Without the .so the Kotlin wrapper still compiles; open/create fail with
    // EglError — there is no mock core.
    val rustSo = file("src/main/jniLibs/arm64-v8a/libegl_library.so")
    val enableNativeFfi =
        rustSo.exists() || (findProperty("egl.ffi")?.toString() == "true")

    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
        buildConfigField("boolean", "HAS_RUST_CORE", enableNativeFfi.toString())
        if (enableNativeFfi) {
            ndk {
                abiFilters += listOf("arm64-v8a")
            }
        }
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        buildConfig = true
    }

    if (enableNativeFfi) {
        externalNativeBuild {
            cmake {
                path = file("src/main/cpp/CMakeLists.txt")
            }
        }
    }

    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }
}

dependencies {
    implementation("androidx.annotation:annotation:1.9.1")
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
}

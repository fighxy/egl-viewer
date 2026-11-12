# Keep JNI entry points for the Rust-backed library wrapper.
-keep class app.eglviewer.core.EglNative { *; }
# JNI_OnLoad resolves these by name for callbacks on the core's threads (D-109).
-keepclassmembers class app.eglviewer.core.Library {
    static void dispatchEventFromJni(long, java.lang.String);
}
-keepclassmembers class app.eglviewer.core.PlatformDecoder {
    static int decodeFromJni(byte[], java.lang.String, long);
}

package app.eglviewer.core;

/**
 * Host stand-in for the Kotlin `EglNative` object (scripts/check-android-jni.sh): the same
 * JNI names, so the real egl_jni.cpp and the real core run on a desktop JVM.
 */
final class EglNative {
    private EglNative() {}

    static native String createLibrary(String parent, String name, String appVersion);
    static native long open(String path, String appVersion, boolean syncFolder);
    static native void close(long handle);
    static native String libraryInfo(long handle);
    static native String counts(long handle);
    static native String folderTree(long handle);
    static native int setEventCallback(long handle, boolean enabled);
    static native int lastStatus();
    static native String lastErrorGlobal();
    static native String openView(long handle, String queryJson);
    static native String viewRange(long handle, long viewId, int start, int len);
    static native void closeView(long handle, long viewId);
    static native int editItems(long handle, String targetJson, String editJson);
    static native String tags(long handle);
    static native String addFiles(long handle, String requestJson);
    static native String importStatus(long handle, long id);
    static native int setPlatformDecoder(boolean enabled);
}

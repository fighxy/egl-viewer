package app.eglviewer.core;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * Host stand-in for `Library.dispatchEventFromJni`: records what the core sends and, like
 * OpenLibrary, reads the library again from inside the callback.
 */
public final class Library {
    static final List<String> EVENTS = new CopyOnWriteArrayList<>();
    static final List<String> THREADS = new CopyOnWriteArrayList<>();
    /** Reads from inside a callback that came back empty (a deadlock would hang instead). */
    static final List<String> FAILED_READS = new CopyOnWriteArrayList<>();

    private Library() {}

    public static void dispatchEventFromJni(long handle, String json) {
        if (EglNative.counts(handle) == null || EglNative.folderTree(handle) == null) {
            FAILED_READS.add(json);
        }
        EVENTS.add(json);
        THREADS.add(Thread.currentThread().getName());
    }
}

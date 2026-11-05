package app.eglviewer.core;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/** Host stand-in for `Library.dispatchEventFromJni`: records what the core sends. */
public final class Library {
    static final List<String> EVENTS = new CopyOnWriteArrayList<>();
    static final List<String> THREADS = new CopyOnWriteArrayList<>();

    private Library() {}

    public static void dispatchEventFromJni(long handle, String json) {
        EVENTS.add(json);
        THREADS.add(Thread.currentThread().getName());
    }
}

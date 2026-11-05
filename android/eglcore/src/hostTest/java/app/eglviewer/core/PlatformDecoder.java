package app.eglviewer.core;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/** Host stand-in for `PlatformDecoder.decodeFromJni`: records the call, decodes nothing. */
public final class PlatformDecoder {
    static final List<String> CALLS = new CopyOnWriteArrayList<>();

    private PlatformDecoder() {}

    public static int decodeFromJni(byte[] bytes, String ext, long sinkPtr) {
        CALLS.add(ext + ":" + bytes.length);
        return 0;
    }
}

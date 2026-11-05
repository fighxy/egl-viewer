package app.eglviewer.core;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * The JNI bridge against the real core on a desktop JVM (scripts/check-android-jni.sh).
 * Runner loads this class through its own class loader, as Android loads app classes, so
 * a FindClass on a thread the core started would miss them (D-109).
 */
public final class HostJniTest {
    private static final List<String> FAILURES = new ArrayList<>();

    private static void check(boolean ok, String what) {
        System.out.println((ok ? "ok   " : "FAIL ") + what);
        if (!ok) FAILURES.add(what);
    }

    /** JSON string literal, enough for names and tags. */
    private static String quote(String s) {
        return "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    }

    private static void waitFor(long handle, long id) throws InterruptedException {
        long deadline = System.currentTimeMillis() + 60_000;
        while (System.currentTimeMillis() < deadline) {
            String status = EglNative.importStatus(handle, id);
            if (status != null && (status.contains("\"state\":\"done\"") || status.contains("\"state\":\"cancelled\""))) {
                return;
            }
            Thread.sleep(50);
        }
        check(false, "the import finishes");
    }

    public static int run(Path dir, Path png) throws Exception {
        System.loadLibrary("egl_jni");
        String name = "Фото 🎨 «тест»";
        String path = EglNative.createLibrary(dir.toString(), name, "0.0.0-test");
        check(path != null, "a library named with an emoji is created (" + EglNative.lastErrorGlobal() + ")");
        if (path == null) return finish();
        check(path.endsWith(name + ".eglib"), "its folder keeps the name: " + path);
        long handle = EglNative.open(path, "0.0.0-test", false);
        check(handle != 0, "it opens (" + EglNative.lastErrorGlobal() + ")");
        if (handle == 0) return finish();
        String info = EglNative.libraryInfo(handle);
        check(info != null && info.contains(quote(name)), "its info names it: " + info);

        EglNative.setEventCallback(handle, true);
        EglNative.setPlatformDecoder(true);
        Path files = Files.createDirectories(dir.resolve("in"));
        Files.copy(png, files.resolve("кот 🐱.png"));
        // Not a real HEIC: the core hands it to the platform decoder, which says no.
        Files.write(files.resolve("снимок.heic"), "not really heic".getBytes(StandardCharsets.UTF_8));
        String ticket = EglNative.addFiles(handle, "{\"paths\":[" + quote(files.toString()) + "],\"folder\":null}");
        check(ticket != null, "the import starts (" + EglNative.lastErrorGlobal() + ")");
        if (ticket == null) return finish();
        long id = Long.parseLong(ticket.replaceAll("[^0-9]", ""));
        waitFor(handle, id);

        boolean fromCore = false;
        for (String thread : Library.THREADS) {
            if (!thread.equals(Thread.currentThread().getName())) fromCore = true;
        }
        check(!Library.EVENTS.isEmpty() && fromCore,
            "events reach the app from the core's threads (" + Library.EVENTS.size() + " events)");
        boolean progress = false;
        for (String event : Library.EVENTS) progress |= event.contains("\"importProgress\"");
        check(progress, "import progress arrives as an event");
        check(Library.FAILED_READS.isEmpty(), "the callback can read the library again: " + Library.FAILED_READS);
        check(!PlatformDecoder.CALLS.isEmpty(), "the platform decoder is asked for the HEIC: " + PlatformDecoder.CALLS);

        String opened = EglNative.openView(handle, "{\"sort\":\"name\",\"ascending\":true}");
        check(opened != null, "a view opens");
        long view = Long.parseLong(opened.replaceAll(".*\"viewId\":([0-9]+).*", "$1"));
        String cells = EglNative.viewRange(handle, view, 0, 10);
        check(cells != null && cells.contains("\"кот 🐱\""), "the item keeps its emoji name: " + cells);
        String itemId = cells == null ? "" : cells.replaceAll("(?s).*?\"id\":\"([^\"]+)\".*", "$1");
        int edited = EglNative.editItems(handle,
            "{\"all\":false,\"ids\":[" + quote(itemId) + "]}",
            "{\"type\":\"addTags\",\"value\":[\"🔥 Огонь\",\"fire\"]}");
        check(edited == 1, "an emoji tag is added to the item (" + edited + ")");
        boolean invalidated = false;
        for (String event : Library.EVENTS) invalidated |= event.contains("\"viewsInvalidated\"");
        check(invalidated && Library.FAILED_READS.isEmpty(), "an edit refreshes the open view, read again in the callback");
        String tags = EglNative.tags(handle);
        check(tags != null && tags.contains("\"🔥 Огонь\""), "the tag comes back as typed: " + tags);
        EglNative.closeView(handle, view);

        // D-111: an encrypted export opens again through the bridge, an emoji passphrase too.
        String secret = "пароль 🔑 длинный";
        Path sealed = Files.createDirectories(dir.resolve("sealed"));
        String started = EglNative.startExport(handle, "{\"selection\":{\"whole\":true,\"folders\":[],\"tags\":[]},"
            + "\"includeSubfolders\":true,\"format\":{\"kind\":\"zip\",\"compression\":\"none\"},"
            + "\"destination\":" + quote(sealed.toString()) + ",\"encrypt\":" + quote(secret) + ",\"eagle\":null}");
        check(started != null, "an encrypted export starts (" + EglNative.lastErrorGlobal() + ")");
        long deadline = System.currentTimeMillis() + 60_000;
        while (!"[]".equals(EglNative.exports(handle)) && System.currentTimeMillis() < deadline) {
            Thread.sleep(50);
        }
        Path archive = null;
        try (var listing = Files.list(sealed)) {
            archive = listing.filter(entry -> entry.toString().endsWith(".zip.age")).findFirst().orElse(null);
        }
        check(archive != null, "the export wrote a .zip.age");
        if (archive != null) {
            Path unsealed = Files.createDirectories(dir.resolve("opened"));
            String wrong = EglNative.decryptArchive(archive.toString(), unsealed.toString(), "not the passphrase");
            check(wrong == null && EglNative.lastErrorGlobal().contains("Wrong passphrase"),
                "a wrong passphrase is refused: " + EglNative.lastErrorGlobal());
            String right = EglNative.decryptArchive(archive.toString(), unsealed.toString(), secret);
            check(right != null && right.contains(".zip\""), "the archive decrypts to a ZIP: " + right);
        }
        EglNative.setPlatformDecoder(false);
        EglNative.close(handle);
        return finish();
    }

    private static int finish() {
        System.out.println(FAILURES.isEmpty() ? "android JNI host check OK" : FAILURES.size() + " failed");
        return FAILURES.size();
    }
}

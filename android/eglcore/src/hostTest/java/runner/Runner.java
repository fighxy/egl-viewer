package runner;

import java.io.File;
import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Path;

/**
 * Loads the app classes through a class loader of their own, like Android does: the system
 * class loader, which a native thread's FindClass falls back to, cannot see them.
 */
public final class Runner {
    public static void main(String[] args) throws Exception {
        URL app = new File(args[0]).toURI().toURL();
        try (URLClassLoader loader = new URLClassLoader(new URL[] {app}, Runner.class.getClassLoader().getParent())) {
            Class<?> test = Class.forName("app.eglviewer.core.HostJniTest", true, loader);
            Method run = test.getMethod("run", Path.class, Path.class);
            int failed = (int) run.invoke(null, Path.of(args[1]), Path.of(args[2]));
            System.exit(failed == 0 ? 0 : 1);
        }
    }
}

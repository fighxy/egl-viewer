package app.eglviewer.core

import java.io.File
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicReference

/**
 * Typed Kotlin face of the C ABI (`crates/library-ffi`), one [Library] per open `.eglib`.
 * Mirrors Swift `EglCore.Library` for the A1 surface: create / open / close, info, counts,
 * folder tree, and an event-callback stub. Calls may block — run them off the main thread.
 */
class Library private constructor(private var handle: Long) : AutoCloseable {
    private val eventHandler = AtomicReference<((LibraryEvent) -> Unit)?>(null)

    /** Sets what hears the core's events (`null` clears). May run on a background thread. */
    fun setEventHandler(handler: ((LibraryEvent) -> Unit)?) {
        ensureOpen()
        eventHandler.set(handler)
        val status = EglNative.setEventCallback(handle, handler != null)
        if (status != 0) {
            eventHandler.set(null)
            throw error(status)
        }
        if (handler != null) {
            activeLibraries[handle] = this
        } else {
            activeLibraries.remove(handle)
        }
    }

    fun info(): LibraryInfo {
        ensureOpen()
        val json = EglNative.libraryInfo(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return LibraryInfo.fromJson(json)
    }

    fun counts(): LibraryCounts {
        ensureOpen()
        val json = EglNative.counts(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return LibraryCounts.fromJson(json)
    }

    /** Folder tree as the core's JSON array (A1: raw string for the shell; A2 types it). */
    fun folderTreeJson(): String {
        ensureOpen()
        val json = EglNative.folderTree(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return json
    }

    override fun close() {
        val closing = handle
        if (closing == 0L) return
        handle = 0L
        activeLibraries.remove(closing)
        eventHandler.set(null)
        runCatching { EglNative.setEventCallback(closing, false) }
        EglNative.close(closing)
    }

    private fun ensureOpen() {
        if (handle == 0L) {
            throw EglError(EglError.Code.OTHER, "library is closed")
        }
        if (!EglNative.isAvailable) {
            throw EglError(EglError.Code.UNSUPPORTED, "native core is not linked")
        }
    }

    private fun error(status: Int): EglError {
        val message = if (handle != 0L) {
            EglNative.lastError(handle)
        } else {
            EglNative.lastErrorGlobal()
        }
        return EglError.of(if (status == 0) EglError.Code.OTHER.raw else status, message)
    }

    /** Called from JNI on the thread that produced the event. */
    internal fun dispatchEvent(json: String) {
        val handler = eventHandler.get() ?: return
        val event = LibraryEvent.fromJson(json) ?: return
        handler(event)
    }

    companion object {
        const val APP_VERSION_DEFAULT = "0.2.3-android"

        /** handle → live Library, for the JNI event trampoline. */
        private val activeLibraries = ConcurrentHashMap<Long, Library>()

        val isNativeAvailable: Boolean
            get() = EglNative.isAvailable

        /** Creates `<parent>/<name>.eglib` and returns its absolute path. */
        @JvmStatic
        fun create(parent: File, name: String, appVersion: String = APP_VERSION_DEFAULT): File {
            requireNative()
            val path = EglNative.createLibrary(parent.absolutePath, name, appVersion)
            val status = EglNative.lastStatus()
            if (path.isNullOrEmpty()) {
                throw EglError.of(
                    if (status == 0) EglError.Code.OTHER.raw else status,
                    EglNative.lastErrorGlobal(),
                )
            }
            return File(path)
        }

        /** Opens a library folder; [syncFolder] opens it read-only (cloud / synced places). */
        @JvmStatic
        fun open(
            path: File,
            appVersion: String = APP_VERSION_DEFAULT,
            syncFolder: Boolean = false,
        ): Library {
            requireNative()
            val handle = EglNative.open(path.absolutePath, appVersion, syncFolder)
            if (handle == 0L) {
                throw EglError(
                    EglError.Code.OTHER,
                    EglNative.lastErrorGlobal().ifEmpty { "open failed" },
                )
            }
            return Library(handle)
        }

        @JvmStatic
        fun requireNative() {
            if (!EglNative.isAvailable) {
                throw EglError(
                    EglError.Code.UNSUPPORTED,
                    "native core is not linked (build libegl_library.so with cargo-ndk)",
                )
            }
        }

        @JvmStatic
        fun dispatchEventFromJni(handle: Long, json: String) {
            activeLibraries[handle]?.dispatchEvent(json)
        }
    }
}

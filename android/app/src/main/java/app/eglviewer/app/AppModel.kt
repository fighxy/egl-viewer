package app.eglviewer.app

import android.app.Application
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import app.eglviewer.R
import app.eglviewer.adding.PickedFile
import app.eglviewer.core.EglError
import app.eglviewer.core.Library
import app.eglviewer.core.PlatformDecoder
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/**
 * App state: Welcome or one open library, recent list, last error (D-101…D-104).
 * Every core call runs off the main thread.
 */
class AppModel(application: Application) : AndroidViewModel(application) {
    sealed class Screen {
        data object Welcome : Screen()
        data class LibraryOpen(val open: OpenLibrary) : Screen()
    }

    var screen: Screen by mutableStateOf(Screen.Welcome)
        private set
    var recents: List<RecentLibrary> by mutableStateOf(emptyList())
        private set
    var localLibraries: List<File> by mutableStateOf(emptyList())
        private set
    var error: String? by mutableStateOf(null)

    /**
     * A library outside the app's storage waits for "All files access" (D-111): the screen
     * explains and sends the user to the system setting, and [retryPendingAccess] opens it
     * when the app comes back with the access.
     */
    data class PendingOpen(val file: File, val uri: Uri?)

    var pendingAccess: PendingOpen? by mutableStateOf(null)
        private set
    var busy: Boolean by mutableStateOf(false)
        private set

    val locations = LibraryLocations(application)
    private val recentStore = RecentLibraries(application)
    private val savedViews = OpenLibrary.savedViews(application)
    val appVersion: String =
        runCatching {
            application.packageManager.getPackageInfo(application.packageName, 0).versionName
        }.getOrNull() ?: Library.APP_VERSION_DEFAULT

    val openLibrary: OpenLibrary?
        get() = (screen as? Screen.LibraryOpen)?.open

    val nativeLinked: Boolean
        get() = Library.isNativeAvailable

    init {
        PlatformDecoder.install()
        PickedFile.removeLeftovers(application.cacheDir)
        reloadLists()
    }

    fun reloadLists() {
        recents = recentStore.load()
        localLibraries = locations.localLibraries()
    }

    fun dismissError() {
        error = null
    }

    fun create(named: String) {
        val name = named.trim()
        if (name.isEmpty()) {
            error = string(R.string.error_name_required)
            return
        }
        run {
            locations.ensureRoot()
            val created = withContext(Dispatchers.IO) {
                Library.create(locations.root, name, appVersion)
            }
            openPath(created, uri = null, syncFolder = false)
        }
    }

    fun openUri(uri: Uri) {
        run {
            locations.takePersistablePermission(uri)
            val path = locations.resolveFilesystemPath(uri)
                ?: throw EglError(
                    EglError.Code.STORAGE,
                    string(R.string.error_saf_path),
                )
            val file = File(path)
            if (!locations.canAccess(file)) {
                pendingAccess = PendingOpen(file, uri)
                return@run
            }
            if (!file.exists()) {
                throw EglError(EglError.Code.NOT_FOUND, string(R.string.error_missing_library))
            }
            val synced = locations.isSynced(file, uri)
            openPath(file, uri = uri, syncFolder = synced)
        }
    }

    fun openRecent(recent: RecentLibrary) {
        run {
            val uri = recent.uri?.let(Uri::parse)
            if (uri != null) {
                openUri(uri)
                return@run
            }
            val file = File(recent.path)
            if (!locations.canAccess(file)) {
                pendingAccess = PendingOpen(file, null)
                return@run
            }
            if (!file.exists()) {
                throw EglError(EglError.Code.NOT_FOUND, string(R.string.error_missing_library))
            }
            openPath(file, uri = null, syncFolder = locations.isSynced(file, null))
        }
    }

    fun openLocal(file: File) {
        run {
            openPath(file, uri = null, syncFolder = false)
        }
    }

    /** The app is back: open the waiting library if the access was granted. */
    fun retryPendingAccess() {
        val pending = pendingAccess ?: return
        if (!locations.canAccess(pending.file)) return
        pendingAccess = null
        val uri = pending.uri
        if (uri != null) {
            openUri(uri)
        } else {
            run {
                if (!pending.file.exists()) {
                    throw EglError(EglError.Code.NOT_FOUND, string(R.string.error_missing_library))
                }
                openPath(pending.file, uri = null, syncFolder = locations.isSynced(pending.file, null))
            }
        }
    }

    fun dismissPendingAccess() {
        pendingAccess = null
    }

    fun forget(recent: RecentLibrary) {
        recents = recentStore.forget(recent.uri ?: recent.path)
    }

    fun close() {
        val open = openLibrary ?: return
        screen = Screen.Welcome
        viewModelScope.launch {
            withContext(Dispatchers.IO) { open.close() }
            reloadLists()
        }
    }

    private suspend fun openPath(file: File, uri: Uri?, syncFolder: Boolean) {
        openLibrary?.let { current ->
            screen = Screen.Welcome
            withContext(Dispatchers.IO) { current.close() }
        }
        val library = withContext(Dispatchers.IO) {
            Library.open(file, appVersion, syncFolder = syncFolder)
        }
        val open = OpenLibrary(library, file, savedViews)
        withContext(Dispatchers.IO) { open.refresh() }
        open.attachEvents(getApplication())
        screen = Screen.LibraryOpen(open)
        val hint = locations.locationHint(file, uri)
        recents = recentStore.remember(
            RecentLibrary(
                name = open.info.name,
                path = file.absolutePath,
                locationHint = hint,
                uri = uri?.toString(),
            ),
        )
        reloadLists()
    }

    private fun run(work: suspend () -> Unit) {
        viewModelScope.launch {
            busy = true
            try {
                work()
            } catch (error: Throwable) {
                this@AppModel.error = describe(error)
            } finally {
                busy = false
            }
        }
    }

    private fun string(id: Int): String = getApplication<Application>().getString(id)

    fun describe(error: Throwable): String {
        if (error is EglError) {
            return when (error.code) {
                EglError.Code.NOT_A_LIBRARY -> string(R.string.error_not_a_library)
                EglError.Code.READ_ONLY -> string(R.string.error_read_only)
                EglError.Code.WRONG_PASSWORD -> string(R.string.error_wrong_password)
                EglError.Code.UNSUPPORTED ->
                    if (error.message.isNotEmpty()) error.message else string(R.string.error_core_missing)
                else -> error.message.ifEmpty { string(R.string.error_generic) }
            }
        }
        return error.message?.ifEmpty { null } ?: string(R.string.error_generic)
    }
}


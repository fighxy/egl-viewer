package app.eglviewer

import android.app.Application
import app.eglviewer.theme.ThemePreference

/** Applies the stored System / Light / Dark preference before any activity starts (D-119). */
class EglApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        ThemePreference.applyStored(this)
    }
}

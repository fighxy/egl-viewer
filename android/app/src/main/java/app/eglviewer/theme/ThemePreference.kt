package app.eglviewer.theme

import android.content.Context
import androidx.appcompat.app.AppCompatDelegate

/**
 * App theme override — same three choices as the desktop (D-028 / D-119):
 * System, Light, Dark. Stored in SharedPreferences; applied with
 * [AppCompatDelegate.setDefaultNightMode].
 */
enum class ThemePreference(val wire: String) {
    System("system"),
    Light("light"),
    Dark("dark"),
    ;

    fun nightMode(): Int = when (this) {
        System -> AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM
        Light -> AppCompatDelegate.MODE_NIGHT_NO
        Dark -> AppCompatDelegate.MODE_NIGHT_YES
    }

    companion object {
        const val PREFS_NAME = "egl_theme"
        const val KEY = "theme"

        fun fromWire(value: String?): ThemePreference = when (value) {
            Light.wire -> Light
            Dark.wire -> Dark
            else -> System
        }

        fun load(context: Context): ThemePreference {
            val prefs = context.applicationContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            return fromWire(prefs.getString(KEY, System.wire))
        }

        fun save(context: Context, preference: ThemePreference) {
            context.applicationContext
                .getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
                .edit()
                .putString(KEY, preference.wire)
                .apply()
        }

        /** Apply the stored preference (call from Application / Activity startup). */
        fun applyStored(context: Context) {
            apply(load(context))
        }

        fun apply(preference: ThemePreference) {
            AppCompatDelegate.setDefaultNightMode(preference.nightMode())
        }

        /** Persist and apply; AppCompat recreates activities when the night mode changes. */
        fun set(context: Context, preference: ThemePreference) {
            save(context, preference)
            apply(preference)
        }
    }
}

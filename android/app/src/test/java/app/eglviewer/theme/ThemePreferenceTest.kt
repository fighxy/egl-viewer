package app.eglviewer.theme

import androidx.appcompat.app.AppCompatDelegate
import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

/** Host JVM tests for System / Light / Dark (D-119). */
class ThemePreferenceTest {
    @Test
    fun wireValuesMatchDesktop() {
        assertEquals("system", ThemePreference.System.wire)
        assertEquals("light", ThemePreference.Light.wire)
        assertEquals("dark", ThemePreference.Dark.wire)
    }

    @Test
    fun fromWireDefaultsToSystem() {
        assertEquals(ThemePreference.System, ThemePreference.fromWire(null))
        assertEquals(ThemePreference.System, ThemePreference.fromWire(""))
        assertEquals(ThemePreference.System, ThemePreference.fromWire("nope"))
        assertEquals(ThemePreference.Light, ThemePreference.fromWire("light"))
        assertEquals(ThemePreference.Dark, ThemePreference.fromWire("dark"))
        assertEquals(ThemePreference.System, ThemePreference.fromWire("system"))
    }

    @Test
    fun nightModeMatchesAppCompat() {
        assertEquals(AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM, ThemePreference.System.nightMode())
        assertEquals(AppCompatDelegate.MODE_NIGHT_NO, ThemePreference.Light.nightMode())
        assertEquals(AppCompatDelegate.MODE_NIGHT_YES, ThemePreference.Dark.nightMode())
    }

    @Test
    fun lightAndDarkPalettesDifferOnChromeRoles() {
        assertNotEquals(LightEglColors.background, DarkEglColors.background)
        assertNotEquals(LightEglColors.foreground, DarkEglColors.foreground)
        assertNotEquals(LightEglColors.primary, DarkEglColors.primary)
        assertNotEquals(LightEglColors.muted, DarkEglColors.muted)
        assertEquals(Color.White, LightEglColors.background)
        // Preview stage is a constant on EglTokens, not a light/dark pair.
        assertEquals(Color(0xFF0A0A0A), EglTokens.PreviewStage)
    }
}

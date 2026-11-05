package app.eglviewer.locks

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Host JVM tests of the app login (Android A6 ≈ D-098 / D-106). */
class AppLockTest {
    @Test
    fun aHashMatchesOnlyItsPassword() {
        val hash = LoginHash.make("1234 🔐", iterations = 1_000)
        assertTrue(hash.matches("1234 🔐"))
        assertFalse(hash.matches("1234"))
        assertFalse(hash.matches(""))
        assertNotEquals(LoginHash.make("1234 🔐", iterations = 1_000).salt.contentHashCode(), hash.salt.contentHashCode())
    }

    @Test
    fun hashRoundTripsThroughJson() {
        val hash = LoginHash.make("secret", iterations = 1_000)
        val restored = LoginHash.fromJson(hash.toJson())
        assertNotNull(restored)
        assertTrue(restored!!.matches("secret"))
        assertEquals(hash.iterations, restored.iterations)
    }

    @Test
    fun theAppLockSetsChangesAndTurnsOff() {
        val lock = AppLock.memory(iterations = 1_000)
        assertFalse(lock.enabled)
        lock.lock()
        assertFalse(lock.locked)

        lock.setPassword("1234")
        lock.lock()
        assertTrue(lock.locked)
        assertFalse(lock.unlock("0000"))
        assertTrue(lock.locked)
        assertTrue(lock.unlock("1234"))
        assertFalse(lock.locked)

        assertFalse(lock.change(current = "0000", to = "5678"))
        assertTrue(lock.change(current = "1234", to = "5678"))
        assertFalse(lock.turnOff("1234"))
        assertTrue(lock.turnOff("5678"))
        assertFalse(lock.enabled)
    }

    @Test
    fun wrongTriesMakeTheNextOneWait() {
        val lock = AppLock.memory(iterations = 1_000)
        lock.setPassword("1234")
        lock.lock()
        repeat(5) {
            assertFalse(lock.unlock("0000"))
        }
        val wait = lock.waitUntilMs
        assertNotNull(wait)
        assertTrue(wait!! > System.currentTimeMillis())
        assertFalse(lock.unlock("1234"))
        assertTrue(lock.locked)
    }

    /** D-111: restarting the app does not reset the wait after wrong tries. */
    @Test
    fun theWaitSurvivesARestart() {
        val store = MemoryLoginStore()
        val first = AppLock(store = store, iterations = 1_000)
        first.setPassword("1234")
        repeat(5) { assertFalse(first.unlock("0000")) }
        val wait = first.waitUntilMs
        assertNotNull(wait)

        val restarted = AppLock(store = store, iterations = 1_000)
        assertEquals(wait, restarted.waitUntilMs)
        assertFalse("still waiting", restarted.unlock("1234"))
        assertEquals(Attempts(5, wait), store.loadAttempts())
    }

    @Test
    fun aRightPasswordClearsTheWrongTries() {
        val store = MemoryLoginStore()
        val lock = AppLock(store = store, iterations = 1_000)
        lock.setPassword("1234")
        repeat(3) { assertFalse(lock.unlock("0000")) }
        assertEquals(3, store.loadAttempts().failures)
        assertTrue(lock.unlock("1234"))
        assertEquals(Attempts(), store.loadAttempts())
    }

    @Test
    fun memoryStoreKeepsTheLogin() {
        val store = MemoryLoginStore()
        store.saveHash(LoginHash.make("1234", iterations = 1_000))
        assertTrue(store.loadHash()?.matches("1234") == true)
        assertFalse(store.hasBiometricToken)
        store.saveBiometricToken(true)
        assertTrue(store.hasBiometricToken)
        store.saveHash(null)
        assertNull(store.loadHash())
    }

    @Test
    fun recoveryKeyParses() {
        val key = app.eglviewer.core.RecoveryKey.fromJson(
            """{"key":"abc","phrase":"one two three four five six seven eight nine ten eleven twelve"}""",
        )
        assertEquals("abc", key.key)
        assertEquals(12, key.phrase.split(" ").size)
    }
}

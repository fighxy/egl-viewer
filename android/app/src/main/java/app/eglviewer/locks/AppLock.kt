package app.eglviewer.locks

import android.content.Context
import android.content.SharedPreferences
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.PBEKeySpec
import kotlin.math.max
import org.json.JSONObject

/**
 * Where the app's login lives: a salted PBKDF2 hash of the password, and a token that only
 * biometrics can read (Android A6 ≈ D-098 / D-106). SharedPreferences + Keystore on the phone;
 * memory in tests.
 */
interface LoginStore {
    fun loadHash(): LoginHash?
    fun saveHash(hash: LoginHash?)
    val hasBiometricToken: Boolean
    fun saveBiometricToken(on: Boolean)
    /** Starts a biometric prompt; [onResult] is true when the face / finger matched. */
    fun authenticateBiometric(
        activity: FragmentActivity,
        reason: String,
        onResult: (Boolean) -> Unit,
    )
}

/** PBKDF2-HMAC-SHA256 of the password with its own salt. */
data class LoginHash(
    val salt: ByteArray,
    val hash: ByteArray,
    val iterations: Int,
) {
    fun matches(password: String): Boolean {
        val given = derive(password, salt, iterations)
        if (given.size != hash.size) return false
        var diff = 0
        for (i in given.indices) {
            diff = diff or (given[i].toInt() xor hash[i].toInt())
        }
        return diff == 0
    }

    fun toJson(): String =
        JSONObject()
            .put("salt", java.util.Base64.getEncoder().encodeToString(salt))
            .put("hash", java.util.Base64.getEncoder().encodeToString(hash))
            .put("iterations", iterations)
            .toString()

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is LoginHash) return false
        return iterations == other.iterations &&
            salt.contentEquals(other.salt) &&
            hash.contentEquals(other.hash)
    }

    override fun hashCode(): Int {
        var result = salt.contentHashCode()
        result = 31 * result + hash.contentHashCode()
        result = 31 * result + iterations
        return result
    }

    companion object {
        const val DEFAULT_ITERATIONS = 310_000

        fun make(password: String, iterations: Int = DEFAULT_ITERATIONS): LoginHash {
            val salt = ByteArray(16).also { SecureRandom().nextBytes(it) }
            return LoginHash(salt = salt, hash = derive(password, salt, iterations), iterations = iterations)
        }

        fun fromJson(json: String): LoginHash? =
            runCatching {
                val o = JSONObject(json)
                val decoder = java.util.Base64.getDecoder()
                LoginHash(
                    salt = decoder.decode(o.getString("salt")),
                    hash = decoder.decode(o.getString("hash")),
                    iterations = o.getInt("iterations"),
                )
            }.getOrNull()

        fun derive(password: String, salt: ByteArray, iterations: Int): ByteArray {
            val factory = SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256")
            val spec = PBEKeySpec(password.toCharArray(), salt, iterations, 256)
            return factory.generateSecret(spec).encoded
        }
    }
}

/**
 * The app's lock: whether a login is set, whether the app is locked now, and failed tries.
 */
class AppLock(
    private val store: LoginStore,
    private val iterations: Int = LoginHash.DEFAULT_ITERATIONS,
) {
    var enabled: Boolean by mutableStateOf(store.loadHash() != null)
        private set
    var locked: Boolean by mutableStateOf(store.loadHash() != null)
        private set
    var biometryEnabled: Boolean by mutableStateOf(store.hasBiometricToken)
        private set
    /** Until then the password field waits, after too many wrong tries. */
    var waitUntilMs: Long? by mutableStateOf(null)
        private set

    private var failures = 0

    fun lock() {
        if (enabled) locked = true
    }

    fun setPassword(password: String) {
        store.saveHash(LoginHash.make(password, iterations))
        enabled = true
    }

    /** False when [current] is wrong. */
    fun change(current: String, to: String): Boolean {
        if (!check(current)) return false
        store.saveHash(LoginHash.make(to, iterations))
        return true
    }

    /** Turns the login off; false when [current] is wrong. */
    fun turnOff(current: String): Boolean {
        if (!check(current)) return false
        store.saveBiometricToken(false)
        store.saveHash(null)
        enabled = false
        biometryEnabled = false
        locked = false
        return true
    }

    fun setBiometry(on: Boolean) {
        store.saveBiometricToken(on && enabled)
        biometryEnabled = store.hasBiometricToken
    }

    /** True when the password opened the app. */
    fun unlock(password: String): Boolean {
        if (!check(password)) return false
        locked = false
        return true
    }

    fun unlockWithBiometry(
        activity: FragmentActivity,
        reason: String,
        onResult: (Boolean) -> Unit,
    ) {
        if (!biometryEnabled) {
            onResult(false)
            return
        }
        store.authenticateBiometric(activity, reason) { ok ->
            if (ok) {
                failures = 0
                waitUntilMs = null
                locked = false
            }
            onResult(ok)
        }
    }

    fun canUseBiometry(context: Context): Boolean {
        val manager = BiometricManager.from(context)
        val authenticators = BiometricManager.Authenticators.BIOMETRIC_STRONG
        return manager.canAuthenticate(authenticators) == BiometricManager.BIOMETRIC_SUCCESS
    }

    /**
     * Checks a password, counting wrong tries: from the fifth on, each makes the next wait
     * longer (30 s, 60 s, …).
     */
    private fun check(password: String): Boolean {
        val wait = waitUntilMs
        if (wait != null && wait > System.currentTimeMillis()) return false
        val hash = store.loadHash() ?: return true
        if (hash.matches(password)) {
            failures = 0
            waitUntilMs = null
            return true
        }
        failures += 1
        if (failures >= 5) {
            waitUntilMs = System.currentTimeMillis() + (failures - 4) * 30_000L
        }
        return false
    }

    companion object {
        fun standard(context: Context): AppLock =
            AppLock(store = PrefsLoginStore(context.applicationContext))

        /** In-memory store for JVM tests (never touches prefs / Keystore). */
        fun memory(iterations: Int = 1_000): AppLock =
            AppLock(store = MemoryLoginStore(), iterations = iterations)
    }
}

/** The login in app-private prefs, on this device only. */
class PrefsLoginStore(
    context: Context,
    prefsName: String = "app.eglviewer.login",
) : LoginStore {
    private val prefs: SharedPreferences =
        context.getSharedPreferences(prefsName, Context.MODE_PRIVATE)
    private val appContext = context.applicationContext

    override fun loadHash(): LoginHash? =
        prefs.getString(KEY_HASH, null)?.let(LoginHash::fromJson)

    override fun saveHash(hash: LoginHash?) {
        prefs.edit().apply {
            if (hash == null) remove(KEY_HASH) else putString(KEY_HASH, hash.toJson())
        }.apply()
    }

    override val hasBiometricToken: Boolean
        get() {
            if (!prefs.getBoolean(KEY_BIOMETRY, false)) return false
            return runCatching { biometricSecretKey() != null }.getOrDefault(false)
        }

    /**
     * Turning biometrics on makes a Keystore key that only a strong biometric unlocks; no
     * operation runs with it yet. Encrypting a token here, as before, needed that very
     * authentication and failed outside a prompt (D-109).
     */
    override fun saveBiometricToken(on: Boolean) {
        deleteBiometricKey()
        prefs.edit().putBoolean(KEY_BIOMETRY, false).apply()
        if (!on) return
        createBiometricKey()
        prefs.edit().putBoolean(KEY_BIOMETRY, true).apply()
    }

    override fun authenticateBiometric(
        activity: FragmentActivity,
        reason: String,
        onResult: (Boolean) -> Unit,
    ) {
        if (!hasBiometricToken) {
            onResult(false)
            return
        }
        // The prompt authorises one operation with the key: encrypting a few random bytes
        // succeeds only after a strong biometric matched, and the key exists only while
        // the enrolled biometrics have not changed.
        val cipher = Cipher.getInstance(TRANSFORMATION)
        try {
            cipher.init(Cipher.ENCRYPT_MODE, biometricSecretKey())
        } catch (_: Exception) {
            // Enrollment changed: the Keystore key is invalid now.
            deleteBiometricKey()
            prefs.edit().putBoolean(KEY_BIOMETRY, false).apply()
            onResult(false)
            return
        }
        val executor = ContextCompat.getMainExecutor(activity)
        val prompt = BiometricPrompt(
            activity,
            executor,
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    val crypto = result.cryptoObject?.cipher
                    val ok = if (crypto != null) {
                        runCatching {
                            crypto.doFinal(ByteArray(16).also { SecureRandom().nextBytes(it) })
                            true
                        }.getOrDefault(false)
                    } else {
                        false
                    }
                    onResult(ok)
                }

                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    onResult(false)
                }

                override fun onAuthenticationFailed() {
                    // Keep waiting for another attempt; do not finish yet.
                }
            },
        )
        val info = BiometricPrompt.PromptInfo.Builder()
            .setTitle(appContext.getString(app.eglviewer.R.string.login_title))
            .setSubtitle(reason)
            .setNegativeButtonText(appContext.getString(app.eglviewer.R.string.common_cancel))
            .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG)
            .build()
        prompt.authenticate(info, BiometricPrompt.CryptoObject(cipher))
    }

    private fun createBiometricKey() {
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        val builder = KeyGenParameterSpec.Builder(
            BIOMETRIC_KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setUserAuthenticationRequired(true)
            .setInvalidatedByBiometricEnrollment(true)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            builder.setUserAuthenticationParameters(
                0,
                KeyProperties.AUTH_BIOMETRIC_STRONG,
            )
        }
        generator.init(builder.build())
        generator.generateKey()
    }

    private fun biometricSecretKey(): SecretKey? {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        return keyStore.getKey(BIOMETRIC_KEY_ALIAS, null) as? SecretKey
    }

    private fun deleteBiometricKey() {
        runCatching {
            val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
            if (keyStore.containsAlias(BIOMETRIC_KEY_ALIAS)) {
                keyStore.deleteEntry(BIOMETRIC_KEY_ALIAS)
            }
        }
        prefs.edit().remove(KEY_BIOMETRY_BLOB).apply()
    }

    companion object {
        private const val KEY_HASH = "password"
        private const val KEY_BIOMETRY = "biometry"
        /** The token of earlier builds; removed with the key. */
        private const val KEY_BIOMETRY_BLOB = "biometry_blob"
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val BIOMETRIC_KEY_ALIAS = "app.eglviewer.login.biometry"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
    }
}

/** A login that lives as long as the process: JVM unit tests. */
class MemoryLoginStore : LoginStore {
    private var hash: LoginHash? = null
    private var token = false

    override fun loadHash(): LoginHash? = hash
    override fun saveHash(hash: LoginHash?) {
        this.hash = hash
    }

    override val hasBiometricToken: Boolean get() = token

    override fun saveBiometricToken(on: Boolean) {
        token = on
    }

    override fun authenticateBiometric(
        activity: FragmentActivity,
        reason: String,
        onResult: (Boolean) -> Unit,
    ) {
        onResult(token)
    }
}

/** Seconds left before another try is allowed (0 when not waiting). */
fun AppLock.waitSecondsLeft(nowMs: Long = System.currentTimeMillis()): Int {
    val until = waitUntilMs ?: return 0
    return max(0, ((until - nowMs + 999) / 1000).toInt())
}

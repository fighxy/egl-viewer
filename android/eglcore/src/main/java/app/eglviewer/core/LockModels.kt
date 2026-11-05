package app.eglviewer.core

import org.json.JSONObject

/**
 * Shown once when a folder is locked or the recovery key is renewed; never stored
 * (Android A6 ≈ D-098 / D-106).
 */
data class RecoveryKey(
    val key: String,
    val phrase: String,
) {
    companion object {
        fun fromJson(json: String): RecoveryKey = fromJson(JSONObject(json))

        fun fromJson(o: JSONObject): RecoveryKey =
            RecoveryKey(
                key = o.getString("key"),
                phrase = o.getString("phrase"),
            )
    }
}

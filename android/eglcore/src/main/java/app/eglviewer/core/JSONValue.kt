package app.eglviewer.core

import org.json.JSONArray
import org.json.JSONObject

/**
 * Any JSON, kept as it came: smart folder rules are Eagle's rule language, stored unchanged,
 * so keys this app does not know survive an edit (architecture §16, D-097 / D-105).
 */
sealed class JSONValue {
    data object Null : JSONValue()
    data class Bool(val value: Boolean) : JSONValue()
    data class Number(val value: Double) : JSONValue()
    data class Text(val value: kotlin.String) : JSONValue()
    data class Array(val value: List<JSONValue>) : JSONValue()
    data class Object(val value: Map<kotlin.String, JSONValue>) : JSONValue()

    val string: kotlin.String?
        get() = when (this) {
            is Text -> value
            is Number -> if (value == kotlin.math.round(value)) {
                value.toLong().toString()
            } else {
                value.toString()
            }
            else -> null
        }

    val array: List<JSONValue>?
        get() = (this as? Array)?.value

    val obj: Map<kotlin.String, JSONValue>?
        get() = (this as? Object)?.value

    /** Numbers of a value or a list (strings that hold numbers too). */
    val numbers: List<Double>
        get() = when (this) {
            is Number -> listOf(value)
            is Text -> value.trim().toDoubleOrNull()?.let { listOf(it) }.orEmpty()
            is Array -> value.flatMap { it.numbers }
            else -> emptyList()
        }

    val strings: List<kotlin.String>
        get() = when (this) {
            is Array -> value.mapNotNull { it.string }
            else -> string?.let { listOf(it) }.orEmpty()
        }

    fun toJson(): Any? = when (this) {
        Null -> JSONObject.NULL
        is Bool -> value
        is Number ->
            if (value == kotlin.math.round(value) && kotlin.math.abs(value) < 9e15) {
                value.toLong()
            } else {
                value
            }
        is Text -> value
        is Array -> JSONArray().also { arr -> value.forEach { arr.put(toJsonElement(it)) } }
        is Object -> JSONObject().also { o -> value.forEach { (k, v) -> o.put(k, toJsonElement(v)) } }
    }

    companion object {
        fun from(any: Any?): JSONValue = when (any) {
            null, JSONObject.NULL -> Null
            is Boolean -> Bool(any)
            is Int -> Number(any.toDouble())
            is Long -> Number(any.toDouble())
            is Float -> Number(any.toDouble())
            is Double -> Number(any)
            is kotlin.String -> Text(any)
            is JSONArray -> Array((0 until any.length()).map { from(any.get(it)) })
            is JSONObject -> {
                val map = linkedMapOf<kotlin.String, JSONValue>()
                any.keys().forEach { key -> map[key] = from(any.get(key)) }
                Object(map)
            }
            is java.lang.Number -> Number(any.doubleValue())
            else -> Text(any.toString())
        }

        fun parse(json: kotlin.String): JSONValue {
            val trimmed = json.trim()
            return when {
                trimmed.startsWith("{") -> from(JSONObject(trimmed))
                trimmed.startsWith("[") -> from(JSONArray(trimmed))
                trimmed == "null" -> Null
                trimmed == "true" -> Bool(true)
                trimmed == "false" -> Bool(false)
                trimmed.toDoubleOrNull() != null -> Number(trimmed.toDouble())
                trimmed.startsWith("\"") -> Text(JSONObject("{\"v\":$trimmed}").getString("v"))
                else -> Text(trimmed)
            }
        }

        private fun toJsonElement(value: JSONValue): Any? = value.toJson()
    }
}

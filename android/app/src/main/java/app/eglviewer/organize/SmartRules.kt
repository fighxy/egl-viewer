package app.eglviewer.organize

import app.eglviewer.core.JSONValue
import java.util.Calendar
import java.util.UUID

/** Eagle's smart folder rules as the editor sees them (desktop rules.ts, D-097 / D-105). */
enum class RuleKind { SET, TEXT, TYPE, RATING, SHAPE, DATE, NUMBER }

data class SmartRule(
    val id: String = UUID.randomUUID().toString(),
    var fields: MutableMap<String, JSONValue>,
) {
    val property: String get() = fields["property"]?.string.orEmpty()
    val method: String get() = fields["method"]?.string.orEmpty()
    var value: JSONValue?
        get() = fields["value"]
        set(v) {
            if (v == null) fields.remove("value") else fields["value"] = v
        }

    val kind: RuleKind? get() = SmartRules.kindOf(property)

    /** The editor knows the property and the method. */
    val editable: Boolean
        get() {
            val k = kind ?: return false
            return SmartRules.methodsOf(k).any { it == method }
        }
}

data class SmartCondition(
    val id: String = UUID.randomUUID().toString(),
    var fields: MutableMap<String, JSONValue>,
    var rules: MutableList<SmartRule>,
) {
    /** `match: "OR"`: one rule is enough. */
    var any: Boolean
        get() = fields["match"]?.string?.uppercase() == "OR"
        set(v) {
            fields["match"] = JSONValue.Text(if (v) "OR" else "AND")
        }

    /** `boolean: "FALSE"`: the items that do not match. */
    var negated: Boolean
        get() = fields["boolean"]?.string?.uppercase() == "FALSE"
        set(v) {
            fields["boolean"] = JSONValue.Text(if (v) "FALSE" else "TRUE")
        }
}

data class SmartProperty(val property: String, val kind: RuleKind)

object SmartRules {
    /** The properties of the v1 subset, in the order the editor offers them. */
    val properties: List<SmartProperty> = listOf(
        SmartProperty("tags", RuleKind.SET),
        SmartProperty("folders", RuleKind.SET),
        SmartProperty("name", RuleKind.TEXT),
        SmartProperty("annotation", RuleKind.TEXT),
        SmartProperty("url", RuleKind.TEXT),
        SmartProperty("type", RuleKind.TYPE),
        SmartProperty("rating", RuleKind.RATING),
        SmartProperty("shape", RuleKind.SHAPE),
        SmartProperty("createTime", RuleKind.DATE),
        SmartProperty("mtime", RuleKind.DATE),
        SmartProperty("btime", RuleKind.DATE),
        SmartProperty("width", RuleKind.NUMBER),
        SmartProperty("height", RuleKind.NUMBER),
        SmartProperty("fileSize", RuleKind.NUMBER),
    )

    fun methodsOf(kind: RuleKind): List<String> = when (kind) {
        RuleKind.SET -> listOf("union", "intersection", "equal", "identity", "empty", "not-empty")
        RuleKind.TEXT -> listOf("contain", "uncontain", "equal", "startWith", "endWith", "empty", "not-empty")
        RuleKind.TYPE, RuleKind.RATING, RuleKind.SHAPE -> listOf("equal", "unequal")
        RuleKind.DATE -> listOf("on", "before", "after", "between", "within")
        RuleKind.NUMBER -> listOf(">=", "<=", ">", "<", "=", "between")
    }

    fun kindOf(property: String): RuleKind? =
        properties.firstOrNull { it.property == property }?.kind

    fun needsValue(method: String): Boolean =
        method != "empty" && method != "not-empty"

    const val DAY_MILLISECONDS = 86_400_000.0

    /** Today's start in the user's time zone, in milliseconds. */
    fun today(calendar: Calendar = Calendar.getInstance()): Double {
        val c = calendar.clone() as Calendar
        c.set(Calendar.HOUR_OF_DAY, 0)
        c.set(Calendar.MINUTE, 0)
        c.set(Calendar.SECOND, 0)
        c.set(Calendar.MILLISECOND, 0)
        return c.timeInMillis.toDouble()
    }

    fun newRule(property: String): SmartRule {
        val fields = mutableMapOf<String, JSONValue>("property" to JSONValue.Text(property))
        when (kindOf(property)) {
            RuleKind.SET -> {
                fields["method"] = JSONValue.Text("union")
                fields["value"] = JSONValue.Array(emptyList())
            }
            RuleKind.TEXT -> {
                fields["method"] = JSONValue.Text("contain")
                fields["value"] = JSONValue.Text("")
            }
            RuleKind.TYPE -> {
                fields["method"] = JSONValue.Text("equal")
                fields["value"] = JSONValue.Text("image")
            }
            RuleKind.RATING -> {
                fields["method"] = JSONValue.Text("equal")
                fields["value"] = JSONValue.Text("5")
            }
            RuleKind.SHAPE -> {
                fields["method"] = JSONValue.Text("equal")
                fields["value"] = JSONValue.Text("landscape")
            }
            RuleKind.DATE -> {
                fields["method"] = JSONValue.Text("within")
                fields["value"] = JSONValue.Array(listOf(JSONValue.Number(7.0)))
            }
            RuleKind.NUMBER -> {
                fields["method"] = JSONValue.Text(">=")
                if (property == "fileSize") {
                    fields["value"] = JSONValue.Array(listOf(JSONValue.Number(1.0)))
                    fields["unit"] = JSONValue.Text("mb")
                } else {
                    fields["value"] = JSONValue.Array(listOf(JSONValue.Number(1000.0)))
                }
            }
            null -> fields["method"] = JSONValue.Text("")
        }
        return SmartRule(fields = fields)
    }

    fun withProperty(rule: SmartRule, property: String): SmartRule {
        if (property == rule.property) return rule
        return newRule(property)
    }

    /** The rule with another method; the value keeps what still fits. */
    fun withMethod(rule: SmartRule, method: String): SmartRule {
        val next = SmartRule(id = rule.id, fields = rule.fields.toMutableMap())
        next.fields["method"] = JSONValue.Text(method)
        val numbers = rule.value?.numbers.orEmpty()
        when (rule.kind) {
            RuleKind.DATE -> {
                if (method == "within") {
                    next.value = JSONValue.Array(listOf(JSONValue.Number(7.0)))
                } else {
                    val first = if (rule.method == "within") today() else (numbers.firstOrNull() ?: today())
                    next.value = if (method == "between") {
                        JSONValue.Array(
                            listOf(
                                JSONValue.Number(first),
                                JSONValue.Number(
                                    if (numbers.size > 1) numbers[1] else first + 7 * DAY_MILLISECONDS,
                                ),
                            ),
                        )
                    } else {
                        JSONValue.Array(listOf(JSONValue.Number(first)))
                    }
                }
            }
            RuleKind.NUMBER -> {
                val first = numbers.firstOrNull() ?: 0.0
                next.value = if (method == "between") {
                    JSONValue.Array(
                        listOf(
                            JSONValue.Number(first),
                            JSONValue.Number(if (numbers.size > 1) numbers[1] else first),
                        ),
                    )
                } else {
                    JSONValue.Array(listOf(JSONValue.Number(first)))
                }
            }
            else -> Unit
        }
        return next
    }

    /** Eagle's `conditions` array, read leniently; anything else gives no conditions. */
    fun read(value: JSONValue?): List<SmartCondition> {
        val list = value?.array ?: return emptyList()
        return list.mapNotNull { entry ->
            val fields = entry.obj?.toMutableMap() ?: return@mapNotNull null
            val rulesRaw = fields.remove("rules")?.array.orEmpty()
            val rules = rulesRaw.mapNotNull { rule ->
                rule.obj?.toMutableMap()?.let { SmartRule(fields = it) }
            }.toMutableList()
            SmartCondition(fields = fields, rules = rules)
        }
    }

    fun write(conditions: List<SmartCondition>): JSONValue =
        JSONValue.Array(
            conditions.map { condition ->
                val fields = condition.fields.toMutableMap()
                fields["rules"] = JSONValue.Array(condition.rules.map { JSONValue.Object(it.fields) })
                JSONValue.Object(fields)
            },
        )

    fun newCondition(): SmartCondition =
        SmartCondition(
            fields = mutableMapOf(
                "match" to JSONValue.Text("AND"),
                "boolean" to JSONValue.Text("TRUE"),
            ),
            rules = mutableListOf(newRule("tags")),
        )
}

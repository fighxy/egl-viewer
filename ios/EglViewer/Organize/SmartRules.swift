import EglCore
import Foundation

/// Eagle's smart folder rules as the editor sees them, as on the desktop (ui/src/features/smart/
/// rules.ts, D-097). A rule is its JSON object: keys the editor does not know stay as they are,
/// and rules it cannot edit are kept unchanged and shown as such.
enum RuleKind: Equatable {
    case set, text, type, rating, shape, date, number
}

struct SmartRule: Equatable, Identifiable {
    let id = UUID()
    var fields: [String: JSONValue]

    var property: String { fields["property"]?.string ?? "" }
    var method: String { fields["method"]?.string ?? "" }
    var value: JSONValue? {
        get { fields["value"] }
        set { fields["value"] = newValue }
    }

    var kind: RuleKind? { SmartRules.kind(of: property) }

    /// The editor knows the property and the method.
    var editable: Bool {
        guard let kind else { return false }
        return SmartRules.methods(of: kind).contains { $0.method == method }
    }

    static func == (a: Self, b: Self) -> Bool { a.fields == b.fields }
}

struct SmartCondition: Equatable, Identifiable {
    let id = UUID()
    /// The condition's own keys besides `rules` (`match`, `boolean`, and any others).
    var fields: [String: JSONValue]
    var rules: [SmartRule]

    /// `match: "OR"`: one rule is enough.
    var any: Bool {
        get { fields["match"]?.string?.uppercased() == "OR" }
        set { fields["match"] = .string(newValue ? "OR" : "AND") }
    }

    /// `boolean: "FALSE"`: the items that do not match.
    var negated: Bool {
        get { fields["boolean"]?.string?.uppercased() == "FALSE" }
        set { fields["boolean"] = .string(newValue ? "FALSE" : "TRUE") }
    }

    static func == (a: Self, b: Self) -> Bool { a.fields == b.fields && a.rules == b.rules }
}

enum SmartRules {
    struct Property {
        let property: String
        let kind: RuleKind
    }

    /// The properties of the v1 subset, in the order the editor offers them.
    static let properties: [Property] = [
        Property(property: "tags", kind: .set),
        Property(property: "folders", kind: .set),
        Property(property: "name", kind: .text),
        Property(property: "annotation", kind: .text),
        Property(property: "url", kind: .text),
        Property(property: "type", kind: .type),
        Property(property: "rating", kind: .rating),
        Property(property: "shape", kind: .shape),
        Property(property: "createTime", kind: .date),
        Property(property: "mtime", kind: .date),
        Property(property: "btime", kind: .date),
        Property(property: "width", kind: .number),
        Property(property: "height", kind: .number),
        Property(property: "fileSize", kind: .number),
    ]

    struct Method: Hashable {
        let method: String
    }

    static func methods(of kind: RuleKind) -> [Method] {
        let names: [String] = switch kind {
        case .set: ["union", "intersection", "equal", "identity", "empty", "not-empty"]
        case .text: ["contain", "uncontain", "equal", "startWith", "endWith", "empty", "not-empty"]
        case .type, .rating, .shape: ["equal", "unequal"]
        case .date: ["on", "before", "after", "between", "within"]
        case .number: [">=", "<=", ">", "<", "=", "between"]
        }
        return names.map(Method.init)
    }

    static func kind(of property: String) -> RuleKind? {
        properties.first { $0.property == property }?.kind
    }

    /// Methods that take no value.
    static func needsValue(_ method: String) -> Bool {
        method != "empty" && method != "not-empty"
    }

    static let dayMilliseconds = 86_400_000.0

    /// Today's start in the user's time zone, in milliseconds.
    static func today(_ calendar: Calendar = .current) -> Double {
        calendar.startOfDay(for: Date()).timeIntervalSince1970 * 1000
    }

    /// A new rule for `property` with a useful default value.
    static func newRule(_ property: String) -> SmartRule {
        var fields: [String: JSONValue] = ["property": .string(property)]
        switch kind(of: property) {
        case .set:
            fields["method"] = .string("union")
            fields["value"] = .array([])
        case .text:
            fields["method"] = .string("contain")
            fields["value"] = .string("")
        case .type:
            fields["method"] = .string("equal")
            fields["value"] = .string("image")
        case .rating:
            fields["method"] = .string("equal")
            fields["value"] = .string("5")
        case .shape:
            fields["method"] = .string("equal")
            fields["value"] = .string("landscape")
        case .date:
            fields["method"] = .string("within")
            fields["value"] = .array([.number(7)])
        case .number:
            fields["method"] = .string(">=")
            if property == "fileSize" {
                fields["value"] = .array([.number(1)])
                fields["unit"] = .string("mb")
            } else {
                fields["value"] = .array([.number(1000)])
            }
        case nil:
            fields["method"] = .string("")
        }
        return SmartRule(fields: fields)
    }

    /// The rule with another property: a fresh rule of that property.
    static func withProperty(_ rule: SmartRule, _ property: String) -> SmartRule {
        guard property != rule.property else { return rule }
        return newRule(property)
    }

    /// The rule with another method; the value keeps what still fits.
    static func withMethod(_ rule: SmartRule, _ method: String) -> SmartRule {
        var next = rule
        next.fields["method"] = .string(method)
        let numbers = rule.value?.numbers ?? []
        switch rule.kind {
        case .date:
            if method == "within" {
                next.value = .array([.number(7)])
            } else {
                let first = rule.method == "within" ? today() : (numbers.first ?? today())
                next.value = method == "between"
                    ? .array([.number(first), .number(numbers.count > 1 ? numbers[1] : first + 7 * dayMilliseconds)])
                    : .array([.number(first)])
            }
        case .number:
            let first = numbers.first ?? 0
            next.value = method == "between"
                ? .array([.number(first), .number(numbers.count > 1 ? numbers[1] : first)])
                : .array([.number(first)])
        default:
            break
        }
        return next
    }

    /// Eagle's `conditions` array, read leniently; anything else gives no conditions.
    static func read(_ value: JSONValue?) -> [SmartCondition] {
        guard let list = value?.array else { return [] }
        return list.compactMap { entry in
            guard var fields = entry.object else { return nil }
            let rules = (fields.removeValue(forKey: "rules")?.array ?? []).compactMap { rule in
                rule.object.map(SmartRule.init(fields:))
            }
            return SmartCondition(fields: fields, rules: rules)
        }
    }

    /// The conditions back as Eagle's array.
    static func write(_ conditions: [SmartCondition]) -> JSONValue {
        .array(conditions.map { condition in
            var fields = condition.fields
            fields["rules"] = .array(condition.rules.map { .object($0.fields) })
            return .object(fields)
        })
    }

    static func newCondition() -> SmartCondition {
        SmartCondition(fields: ["match": .string("AND"), "boolean": .string("TRUE")], rules: [newRule("tags")])
    }
}

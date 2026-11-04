// Any JSON, kept as it came: smart folder rules are Eagle's rule language, stored unchanged,
// so keys this app does not know survive an edit (docs/architecture.md §16, D-097).

import Foundation

public enum JSONValue: Codable, Equatable, Hashable, Sendable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(Bool.self) {
            self = .bool(value)
        } else if let value = try? container.decode(Double.self) {
            self = .number(value)
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else if let value = try? container.decode([JSONValue].self) {
            self = .array(value)
        } else {
            self = .object(try container.decode([String: JSONValue].self))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .null: try container.encodeNil()
        case let .bool(value): try container.encode(value)
        case let .number(value):
            // Whole numbers go out without a fraction, as Eagle writes them.
            if value.rounded() == value, abs(value) < 9e15 {
                try container.encode(Int64(value))
            } else {
                try container.encode(value)
            }
        case let .string(value): try container.encode(value)
        case let .array(value): try container.encode(value)
        case let .object(value): try container.encode(value)
        }
    }

    public var string: String? {
        switch self {
        case let .string(value): value
        case let .number(value): value.rounded() == value ? String(Int64(value)) : String(value)
        default: nil
        }
    }

    public var array: [JSONValue]? {
        if case let .array(value) = self { return value }
        return nil
    }

    public var object: [String: JSONValue]? {
        if case let .object(value) = self { return value }
        return nil
    }

    /// Numbers of a value or a list, as the core reads them (strings that hold numbers too).
    public var numbers: [Double] {
        switch self {
        case let .number(value): [value]
        case let .string(value): Double(value.trimmingCharacters(in: .whitespaces)).map { [$0] } ?? []
        case let .array(list): list.flatMap(\.numbers)
        default: []
        }
    }

    /// Strings of a value or a list.
    public var strings: [String] {
        switch self {
        case let .array(list): list.compactMap(\.string)
        default: string.map { [$0] } ?? []
        }
    }
}

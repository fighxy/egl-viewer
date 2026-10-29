// Eagle's smart folder rules as the editor sees them (docs/architecture.md §16). The JSON is
// stored unchanged: a rule this editor does not know keeps its keys, and Rust evaluates it.
import type { JsonValue } from "@/ipc";

export type SmartRule = {
  property: string;
  method: string;
  value?: JsonValue;
  unit?: string;
  [key: string]: JsonValue | undefined;
};

export type SmartCondition = {
  match?: string;
  boolean?: string;
  rules: SmartRule[];
  [key: string]: JsonValue | SmartRule[] | undefined;
};

export type RuleKind = "set" | "text" | "type" | "rating" | "shape" | "date" | "number";

/** The properties of the v1 subset, in the order the editor offers them. */
export const PROPERTIES: readonly { property: string; label: string; kind: RuleKind }[] = [
  { property: "tags", label: "Tags", kind: "set" },
  { property: "folders", label: "Folders", kind: "set" },
  { property: "name", label: "Name", kind: "text" },
  { property: "annotation", label: "Note", kind: "text" },
  { property: "url", label: "Link", kind: "text" },
  { property: "type", label: "Type", kind: "type" },
  { property: "rating", label: "Rating", kind: "rating" },
  { property: "shape", label: "Shape", kind: "shape" },
  { property: "createTime", label: "Date added", kind: "date" },
  { property: "mtime", label: "Date modified", kind: "date" },
  { property: "btime", label: "Date created", kind: "date" },
  { property: "width", label: "Width", kind: "number" },
  { property: "height", label: "Height", kind: "number" },
  { property: "fileSize", label: "File size", kind: "number" },
];

export const METHODS: Record<RuleKind, readonly { method: string; label: string }[]> = {
  set: [
    { method: "union", label: "is any of" },
    { method: "intersection", label: "has all of" },
    { method: "equal", label: "is exactly" },
    { method: "identity", label: "has none of" },
    { method: "empty", label: "is empty" },
    { method: "not-empty", label: "is not empty" },
  ],
  text: [
    { method: "contain", label: "contains" },
    { method: "uncontain", label: "does not contain" },
    { method: "equal", label: "is" },
    { method: "startWith", label: "starts with" },
    { method: "endWith", label: "ends with" },
    { method: "empty", label: "is empty" },
    { method: "not-empty", label: "is not empty" },
  ],
  type: [
    { method: "equal", label: "is" },
    { method: "unequal", label: "is not" },
  ],
  rating: [
    { method: "equal", label: "is" },
    { method: "unequal", label: "is not" },
  ],
  shape: [
    { method: "equal", label: "is" },
    { method: "unequal", label: "is not" },
  ],
  date: [
    { method: "on", label: "is on" },
    { method: "before", label: "is before" },
    { method: "after", label: "is after" },
    { method: "between", label: "is between" },
    { method: "within", label: "is in the last" },
  ],
  number: [
    { method: ">=", label: "is at least" },
    { method: "<=", label: "is at most" },
    { method: ">", label: "is more than" },
    { method: "<", label: "is less than" },
    { method: "=", label: "is" },
    { method: "between", label: "is between" },
  ],
};

export function kindOf(property: string): RuleKind | undefined {
  return PROPERTIES.find((entry) => entry.property === property)?.kind;
}

/** The rule can be edited here; the others stay as they are and are listed as unsupported. */
export function editable(rule: SmartRule): boolean {
  const kind = kindOf(rule.property);
  return kind !== undefined && METHODS[kind].some((entry) => entry.method === rule.method);
}

/** Methods that take no value. */
export function needsValue(method: string): boolean {
  return method !== "empty" && method !== "not-empty";
}

const DAY_MS = 86_400_000;

/** Today's start in the user's time zone. */
export function today(): number {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
}

/** `YYYY-MM-DD` for a day start in the user's time zone. */
export function dayText(ms: number): string {
  const date = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** The start of a `YYYY-MM-DD` day in the user's time zone; `null` for no date. */
export function dayStart(text: string): number | null {
  const [year, month, day] = text.split("-").map(Number);
  if (!year || !month || !day) return null;
  return new Date(year, month - 1, day).getTime();
}

/** A new rule for `property` with a useful default value. */
export function newRule(property: string): SmartRule {
  switch (kindOf(property)) {
    case "set":
      return { property, method: "union", value: [] };
    case "text":
      return { property, method: "contain", value: "" };
    case "type":
      return { property, method: "equal", value: "image" };
    case "rating":
      return { property, method: "equal", value: "5" };
    case "shape":
      return { property, method: "equal", value: "landscape" };
    case "date":
      return { property, method: "within", value: [7] };
    case "number":
      return property === "fileSize"
        ? { property, method: ">=", value: [1], unit: "mb" }
        : { property, method: ">=", value: [1000] };
    default:
      return { property, method: "" };
  }
}

/** The rule with another method; the value keeps what still fits. */
export function withMethod(rule: SmartRule, method: string): SmartRule {
  const kind = kindOf(rule.property);
  const numbers = Array.isArray(rule.value)
    ? rule.value.filter((value): value is number => typeof value === "number")
    : [];
  if (kind === "date") {
    if (method === "within") return { ...rule, method, value: [7] };
    const first = rule.method === "within" ? today() : (numbers[0] ?? today());
    return {
      ...rule,
      method,
      value: method === "between" ? [first, numbers[1] ?? first + 7 * DAY_MS] : [first],
    };
  }
  if (kind === "number") {
    const first = numbers[0] ?? 0;
    return {
      ...rule,
      method,
      value: method === "between" ? [first, numbers[1] ?? first] : [first],
    };
  }
  return { ...rule, method };
}

/** Eagle's `conditions` array, read leniently; anything else gives no conditions. */
export function readConditions(value: JsonValue | undefined): SmartCondition[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((condition) => {
    if (typeof condition !== "object" || condition === null || Array.isArray(condition)) return [];
    const rules = Array.isArray(condition.rules) ? condition.rules : [];
    return [
      {
        ...condition,
        rules: rules.flatMap((rule) =>
          typeof rule === "object" && rule !== null && !Array.isArray(rule)
            ? [
                {
                  ...rule,
                  property: typeof rule.property === "string" ? rule.property : "",
                  method: typeof rule.method === "string" ? rule.method : "",
                } as SmartRule,
              ]
            : [],
        ),
      } as SmartCondition,
    ];
  });
}

export function newCondition(): SmartCondition {
  return { match: "AND", boolean: "TRUE", rules: [newRule("tags")] };
}

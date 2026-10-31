// Eagle's smart folder rules as the editor sees them (docs/architecture.md §16). The JSON is
// stored unchanged: a rule this editor does not know keeps its keys, and Rust evaluates it.
import i18n from "@/i18n";
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

type PropertyDef = { property: string; labelKey: string; kind: RuleKind };
type MethodDef = { method: string; labelKey: string };

/** The properties of the v1 subset, in the order the editor offers them. */
const PROPERTY_DEFS: readonly PropertyDef[] = [
  { property: "tags", labelKey: "smart.propertyTags", kind: "set" },
  { property: "folders", labelKey: "smart.propertyFolders", kind: "set" },
  { property: "name", labelKey: "smart.propertyName", kind: "text" },
  { property: "annotation", labelKey: "smart.propertyNote", kind: "text" },
  { property: "url", labelKey: "smart.propertyLink", kind: "text" },
  { property: "type", labelKey: "smart.propertyType", kind: "type" },
  { property: "rating", labelKey: "smart.propertyRating", kind: "rating" },
  { property: "shape", labelKey: "smart.propertyShape", kind: "shape" },
  { property: "createTime", labelKey: "smart.propertyDateAdded", kind: "date" },
  { property: "mtime", labelKey: "smart.propertyDateModified", kind: "date" },
  { property: "btime", labelKey: "smart.propertyDateCreated", kind: "date" },
  { property: "width", labelKey: "smart.propertyWidth", kind: "number" },
  { property: "height", labelKey: "smart.propertyHeight", kind: "number" },
  { property: "fileSize", labelKey: "smart.propertyFileSize", kind: "number" },
];

const METHOD_DEFS: Record<RuleKind, readonly MethodDef[]> = {
  set: [
    { method: "union", labelKey: "smart.methodIsAnyOf" },
    { method: "intersection", labelKey: "smart.methodHasAllOf" },
    { method: "equal", labelKey: "smart.methodIsExactly" },
    { method: "identity", labelKey: "smart.methodHasNoneOf" },
    { method: "empty", labelKey: "smart.methodIsEmpty" },
    { method: "not-empty", labelKey: "smart.methodIsNotEmpty" },
  ],
  text: [
    { method: "contain", labelKey: "smart.methodContains" },
    { method: "uncontain", labelKey: "smart.methodDoesNotContain" },
    { method: "equal", labelKey: "smart.methodIs" },
    { method: "startWith", labelKey: "smart.methodStartsWith" },
    { method: "endWith", labelKey: "smart.methodEndsWith" },
    { method: "empty", labelKey: "smart.methodIsEmpty" },
    { method: "not-empty", labelKey: "smart.methodIsNotEmpty" },
  ],
  type: [
    { method: "equal", labelKey: "smart.methodIs" },
    { method: "unequal", labelKey: "smart.methodIsNot" },
  ],
  rating: [
    { method: "equal", labelKey: "smart.methodIs" },
    { method: "unequal", labelKey: "smart.methodIsNot" },
  ],
  shape: [
    { method: "equal", labelKey: "smart.methodIs" },
    { method: "unequal", labelKey: "smart.methodIsNot" },
  ],
  date: [
    { method: "on", labelKey: "smart.methodIsOn" },
    { method: "before", labelKey: "smart.methodIsBefore" },
    { method: "after", labelKey: "smart.methodIsAfter" },
    { method: "between", labelKey: "smart.methodIsBetween" },
    { method: "within", labelKey: "smart.methodIsInTheLast" },
  ],
  number: [
    { method: ">=", labelKey: "smart.methodIsAtLeast" },
    { method: "<=", labelKey: "smart.methodIsAtMost" },
    { method: ">", labelKey: "smart.methodIsMoreThan" },
    { method: "<", labelKey: "smart.methodIsLessThan" },
    { method: "=", labelKey: "smart.methodIs" },
    { method: "between", labelKey: "smart.methodIsBetween" },
  ],
};

/** Property options with labels resolved for the current locale. */
export const PROPERTIES: readonly { property: string; label: string; kind: RuleKind }[] =
  PROPERTY_DEFS.map((entry) => ({
    property: entry.property,
    kind: entry.kind,
    get label() {
      return i18n.t(entry.labelKey);
    },
  }));

/** Method options with labels resolved for the current locale. */
export const METHODS: Record<RuleKind, readonly { method: string; label: string }[]> = (
  Object.keys(METHOD_DEFS) as RuleKind[]
).reduce(
  (acc, kind) => {
    acc[kind] = METHOD_DEFS[kind].map((entry) => ({
      method: entry.method,
      get label() {
        return i18n.t(entry.labelKey);
      },
    }));
    return acc;
  },
  {} as Record<RuleKind, readonly { method: string; label: string }[]>,
);

export function kindOf(property: string): RuleKind | undefined {
  return PROPERTY_DEFS.find((entry) => entry.property === property)?.kind;
}

/** The rule can be edited here; the others stay as they are and are listed as unsupported. */
export function editable(rule: SmartRule): boolean {
  const kind = kindOf(rule.property);
  return kind !== undefined && METHOD_DEFS[kind].some((entry) => entry.method === rule.method);
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

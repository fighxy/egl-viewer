//! Smart folder rules (docs/architecture.md §16, docs/spec.md §5): Eagle's rule language, stored
//! unchanged as JSON, and the evaluator for the v1 subset. A rule outside the subset never
//! matches and is listed, so the UI can warn about it.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

use crate::edit::MAX_TEXT_UTF16;
use crate::error::SourceError;
use crate::model::ItemLite;
use crate::query::filter::{FolderIndex, shape_of, type_matches};
use crate::query::search::fold;
use crate::query::{LibraryQuery, Scope, TagMode};

/// Eagle's limits (docs/architecture.md §16.1).
pub const MAX_CONDITIONS: usize = 30;
pub const MAX_RULES: usize = 30;
const DAY_MS: i64 = 86_400_000;

/// What a rule can look at: the light item plus the two texts snapshots do not keep.
#[derive(Debug, Clone, Copy)]
pub struct RuleItem<'a> {
    pub item: &'a ItemLite,
    pub annotation: &'a str,
    pub url: &'a str,
}

/// A smart folder's conditions, parsed. Every condition must match (AND across conditions).
#[derive(Debug, Clone, PartialEq)]
pub struct SmartRules {
    conditions: Vec<Condition>,
}

#[derive(Debug, Clone, PartialEq)]
struct Condition {
    /// `match: "OR"`: one rule is enough; otherwise every rule.
    any: bool,
    /// `boolean: "FALSE"` negates the condition.
    negate: bool,
    rules: Vec<Rule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetField {
    Tags,
    Folders,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetMethod {
    Union,
    Intersection,
    Equal,
    Identity,
    Empty,
    NotEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextField {
    Name,
    Annotation,
    Url,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextMethod {
    Equal,
    StartWith,
    EndWith,
    Contain,
    Uncontain,
    Empty,
    NotEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateField {
    /// `createTime`: the date added.
    Added,
    /// `mtime`, falling back to the date added.
    Modified,
    /// `btime`, falling back to the date added.
    Created,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateMethod {
    On,
    Before,
    After,
    Between,
    Within,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberField {
    Width,
    Height,
    FileSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberMethod {
    Eq,
    Ge,
    Le,
    Gt,
    Lt,
    Between,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RatingMethod {
    Equal,
    Unequal,
    Contain,
}

#[derive(Debug, Clone, PartialEq)]
enum Rule {
    Set {
        field: SetField,
        method: SetMethod,
        values: Vec<String>,
    },
    Type {
        equal: bool,
        value: String,
    },
    Rating {
        method: RatingMethod,
        value: String,
    },
    Shape {
        equal: bool,
        value: String,
    },
    /// `value` is folded: names, notes and links compare without case.
    Text {
        field: TextField,
        method: TextMethod,
        value: String,
    },
    Date {
        field: DateField,
        method: DateMethod,
        values: Vec<i64>,
    },
    Number {
        field: NumberField,
        method: NumberMethod,
        values: Vec<f64>,
        /// `fileSize` compares in KB or MB.
        divisor: f64,
    },
    Unsupported {
        property: String,
        method: String,
    },
}

impl SmartRules {
    /// Reads Eagle's `conditions` array. A structure that is not Eagle's is refused; a rule this
    /// build cannot evaluate is kept as unsupported.
    pub fn parse(conditions: &Value) -> Result<Self, SourceError> {
        let not_eagle = || SourceError::Invalid("The rules are not in Eagle's format.".into());
        let list = conditions.as_array().ok_or_else(not_eagle)?;
        let conditions = list
            .iter()
            .map(|condition| {
                let object = condition.as_object().ok_or_else(not_eagle)?;
                let rules = object
                    .get("rules")
                    .and_then(Value::as_array)
                    .ok_or_else(not_eagle)?
                    .iter()
                    .map(|rule| rule.as_object().map(parse_rule).ok_or_else(not_eagle))
                    .collect::<Result<_, _>>()?;
                Ok(Condition {
                    any: text_of(object.get("match")).eq_ignore_ascii_case("OR"),
                    negate: text_of(object.get("boolean")).eq_ignore_ascii_case("FALSE"),
                    rules,
                })
            })
            .collect::<Result<_, SourceError>>()?;
        Ok(Self { conditions })
    }

    /// Rules that match nothing: what stored rules become when they cannot be read.
    pub fn nothing() -> Self {
        Self {
            conditions: vec![Condition {
                any: false,
                negate: false,
                rules: vec![Rule::Unsupported {
                    property: "unreadable rules".into(),
                    method: String::new(),
                }],
            }],
        }
    }

    /// Eagle's limits, checked before a smart folder is saved: 1–30 conditions of 1–30 rules.
    pub fn check_limits(&self) -> Result<(), SourceError> {
        if self.conditions.is_empty() || self.conditions.iter().any(|c| c.rules.is_empty()) {
            return Err(SourceError::Invalid(
                "A smart folder needs at least one rule.".into(),
            ));
        }
        if self.conditions.len() > MAX_CONDITIONS
            || self.conditions.iter().any(|c| c.rules.len() > MAX_RULES)
        {
            return Err(SourceError::Invalid(format!(
                "A smart folder holds at most {MAX_CONDITIONS} conditions of {MAX_RULES} rules."
            )));
        }
        Ok(())
    }

    /// The rules this build cannot evaluate, as `property method`, each once.
    pub fn unsupported(&self) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        for rule in self.conditions.iter().flat_map(|c| &c.rules) {
            if let Rule::Unsupported { property, method } = rule {
                let name = format!("{property} {method}").trim().to_owned();
                if !found.contains(&name) {
                    found.push(name);
                }
            }
        }
        found
    }

    /// Every condition matches (docs/architecture.md §16.1). `now` is for "within the last N
    /// days".
    pub fn matches(&self, item: &RuleItem<'_>, now: i64) -> bool {
        self.conditions.iter().all(|condition| {
            let hit = if condition.any {
                condition.rules.iter().any(|rule| rule.matches(item, now))
            } else {
                condition.rules.iter().all(|rule| rule.matches(item, now))
            };
            hit != condition.negate
        })
    }
}

fn text_of(value: Option<&Value>) -> &str {
    value.and_then(Value::as_str).unwrap_or_default()
}

/// A rule's value as strings: a string, a number, or a list of them.
fn strings(value: Option<&Value>) -> Vec<String> {
    let one = |value: &Value| match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    };
    match value {
        Some(Value::Array(list)) => list.iter().filter_map(one).collect(),
        Some(value) => one(value).into_iter().collect(),
        None => Vec::new(),
    }
}

fn numbers(value: Option<&Value>) -> Vec<f64> {
    strings(value)
        .iter()
        .filter_map(|text| text.trim().parse::<f64>().ok())
        .collect()
}

fn parse_rule(rule: &serde_json::Map<String, Value>) -> Rule {
    let property = text_of(rule.get("property"));
    let method = text_of(rule.get("method"));
    let value = rule.get("value");
    let unsupported = || Rule::Unsupported {
        property: property.to_owned(),
        method: method.to_owned(),
    };
    match property {
        "tags" | "folders" => {
            let method = match method {
                "union" => SetMethod::Union,
                "intersection" => SetMethod::Intersection,
                "equal" => SetMethod::Equal,
                "identity" => SetMethod::Identity,
                "empty" => SetMethod::Empty,
                "not-empty" => SetMethod::NotEmpty,
                _ => return unsupported(),
            };
            Rule::Set {
                field: if property == "tags" {
                    SetField::Tags
                } else {
                    SetField::Folders
                },
                method,
                values: strings(value),
            }
        }
        "type" | "shape" => {
            let equal = match method {
                "equal" => true,
                "unequal" => false,
                _ => return unsupported(),
            };
            let value = strings(value).into_iter().next().unwrap_or_default();
            if property == "type" {
                Rule::Type { equal, value }
            } else if value == "custom" {
                // A custom ratio compares with the rule's own width and height; not in v1.
                unsupported()
            } else {
                Rule::Shape { equal, value }
            }
        }
        "rating" => {
            let method = match method {
                "equal" => RatingMethod::Equal,
                "unequal" => RatingMethod::Unequal,
                "contain" => RatingMethod::Contain,
                _ => return unsupported(),
            };
            Rule::Rating {
                method,
                value: strings(value).join(","),
            }
        }
        "name" | "annotation" | "url" => {
            let method = match method {
                "equal" | "is" => TextMethod::Equal,
                "startWith" => TextMethod::StartWith,
                "endWith" => TextMethod::EndWith,
                "contain" => TextMethod::Contain,
                "uncontain" => TextMethod::Uncontain,
                "empty" => TextMethod::Empty,
                "not-empty" => TextMethod::NotEmpty,
                _ => return unsupported(),
            };
            Rule::Text {
                field: match property {
                    "name" => TextField::Name,
                    "annotation" => TextField::Annotation,
                    _ => TextField::Url,
                },
                method,
                value: fold(&strings(value).into_iter().next().unwrap_or_default()),
            }
        }
        "createTime" | "mtime" | "btime" => {
            let method = match method {
                "on" => DateMethod::On,
                "before" => DateMethod::Before,
                "after" => DateMethod::After,
                "between" => DateMethod::Between,
                "within" => DateMethod::Within,
                _ => return unsupported(),
            };
            Rule::Date {
                field: match property {
                    "createTime" => DateField::Added,
                    "mtime" => DateField::Modified,
                    _ => DateField::Created,
                },
                method,
                values: numbers(value).into_iter().map(|n| n as i64).collect(),
            }
        }
        "width" | "height" | "fileSize" => {
            let method = match method {
                "=" => NumberMethod::Eq,
                ">=" => NumberMethod::Ge,
                "<=" => NumberMethod::Le,
                ">" => NumberMethod::Gt,
                "<" => NumberMethod::Lt,
                "between" => NumberMethod::Between,
                _ => return unsupported(),
            };
            let divisor = match (property, text_of(rule.get("unit"))) {
                ("fileSize", "kb") => 1024.0,
                ("fileSize", _) => 1024.0 * 1024.0,
                _ => 1.0,
            };
            Rule::Number {
                field: match property {
                    "width" => NumberField::Width,
                    "height" => NumberField::Height,
                    _ => NumberField::FileSize,
                },
                method,
                values: numbers(value),
                divisor,
            }
        }
        _ => unsupported(),
    }
}

impl Rule {
    fn matches(&self, rule_item: &RuleItem<'_>, now: i64) -> bool {
        let item = rule_item.item;
        match self {
            Self::Set {
                field,
                method,
                values,
            } => {
                // An item without the key never matches, not even `empty` (Eagle).
                let own: Vec<&str> = match field {
                    SetField::Tags => item.tags.iter().map(AsRef::as_ref).collect(),
                    SetField::Folders => match &item.folders {
                        Some(folders) => folders.iter().map(String::as_str).collect(),
                        None => return false,
                    },
                };
                let has = |value: &String| own.contains(&value.as_str());
                match method {
                    SetMethod::Union => values.iter().any(has),
                    SetMethod::Intersection => {
                        // An empty value never matches tags, and matches every item with folders.
                        (!values.is_empty() || *field == SetField::Folders)
                            && values.iter().all(has)
                    }
                    SetMethod::Equal => {
                        !values.is_empty()
                            && values.iter().all(has)
                            && own.len() == distinct(values)
                    }
                    SetMethod::Identity => !values.iter().any(has),
                    SetMethod::Empty => own.is_empty(),
                    SetMethod::NotEmpty => !own.is_empty(),
                }
            }
            Self::Type { equal, value } => {
                !value.is_empty() && type_matches(&item.ext, value) == *equal
            }
            Self::Rating { method, value } => {
                let own = if item.star == 0 {
                    "none".to_owned()
                } else {
                    item.star.to_string()
                };
                match method {
                    RatingMethod::Equal => *value == own,
                    RatingMethod::Unequal => *value != own,
                    // "3,4,5" holds 4; a value with "none" matches every item.
                    RatingMethod::Contain => {
                        value.contains("none") || (item.star > 0 && value.contains(&own))
                    }
                }
            }
            Self::Shape { equal, value } => {
                let shape = shape_of(item.width, item.height);
                if *equal {
                    shape == Some(value.as_str())
                } else {
                    shape != Some(value.as_str())
                }
            }
            Self::Text {
                field,
                method,
                value,
            } => {
                let own = fold(match field {
                    TextField::Name => &item.name,
                    TextField::Annotation => rule_item.annotation,
                    TextField::Url => rule_item.url,
                });
                match method {
                    TextMethod::Empty => own.is_empty(),
                    TextMethod::NotEmpty => !own.is_empty(),
                    // Any other method with an empty value matches nothing.
                    _ if value.is_empty() => false,
                    TextMethod::Equal => own == *value,
                    TextMethod::StartWith => own.starts_with(value.as_str()),
                    TextMethod::EndWith => own.ends_with(value.as_str()),
                    TextMethod::Contain => own.contains(value.as_str()),
                    TextMethod::Uncontain => !own.contains(value.as_str()),
                }
            }
            Self::Date {
                field,
                method,
                values,
            } => {
                let at = match field {
                    DateField::Added => item.added_at,
                    DateField::Modified => item.file_modified_at.or(item.added_at),
                    DateField::Created => item.file_created_at.or(item.added_at),
                };
                let (Some(at), Some(&first)) = (at, values.first()) else {
                    return false;
                };
                match method {
                    // The value is the start of a day in the user's time zone.
                    DateMethod::On => first <= at && at < first + DAY_MS,
                    // The day of the value counts (Eagle: t <= value + 1 day).
                    DateMethod::Before => at <= first + DAY_MS,
                    DateMethod::After => at >= first,
                    DateMethod::Between => values
                        .get(1)
                        .is_some_and(|&last| first <= at && at <= last + DAY_MS),
                    DateMethod::Within => at + first * DAY_MS >= now,
                }
            }
            Self::Number {
                field,
                method,
                values,
                divisor,
            } => {
                let own = match field {
                    NumberField::Width => item.width.map(f64::from),
                    NumberField::Height => item.height.map(f64::from),
                    NumberField::FileSize => item.size.map(|bytes| bytes as f64 / divisor),
                };
                let (Some(own), Some(&first)) = (own, values.first()) else {
                    return false;
                };
                match method {
                    NumberMethod::Eq => own == first,
                    NumberMethod::Ge => own >= first,
                    NumberMethod::Le => own <= first,
                    NumberMethod::Gt => own > first,
                    NumberMethod::Lt => own < first,
                    NumberMethod::Between => values
                        .get(1)
                        .is_some_and(|&last| first <= own && own <= last),
                }
            }
            Self::Unsupported { .. } => false,
        }
    }
}

fn distinct(values: &[String]) -> usize {
    let mut seen: Vec<&String> = Vec::with_capacity(values.len());
    for value in values {
        if !seen.contains(&value) {
            seen.push(value);
        }
    }
    seen.len()
}

/// "Save as smart folder" (docs/spec.md §5): the rules that keep what the screen shows, and the
/// smart folder they go under when the screen is one. Search text and the trash cannot be
/// expressed in rules and are refused with the reason.
pub fn rules_from_query(
    query: &LibraryQuery,
    folders: &FolderIndex,
) -> Result<(Option<String>, Value), SourceError> {
    if query
        .text
        .as_deref()
        .is_some_and(|text| !text.trim().is_empty())
    {
        return Err(SourceError::Invalid(
            "Search text cannot go into a smart folder: Eagle's rules have no search. Clear the \
             search first."
                .into(),
        ));
    }
    let mut rules: Vec<Value> = Vec::new();
    let mut parent = None;
    match &query.scope {
        Scope::All => {}
        Scope::Folder {
            id,
            include_descendants,
        } => {
            let mut ids: Vec<String> = if *include_descendants {
                folders.descendant_ids(id).into_iter().collect()
            } else {
                vec![id.clone()]
            };
            ids.sort();
            rules.push(json!({ "property": "folders", "method": "union", "value": ids }));
        }
        Scope::Unfiled => {
            rules.push(json!({ "property": "folders", "method": "empty", "value": [] }));
        }
        Scope::Untagged => {
            rules.push(json!({ "property": "tags", "method": "empty", "value": [] }));
        }
        Scope::Smart { id } => parent = Some(id.clone()),
        Scope::Trash => {
            return Err(SourceError::Invalid(
                "Smart folders leave out the trash.".into(),
            ));
        }
    }
    let set_method = |mode: TagMode| match mode {
        TagMode::Any => "union",
        TagMode::All => "intersection",
        TagMode::Exact => "equal",
        TagMode::None => "identity",
    };
    if !query.tags.is_empty() {
        rules.push(
            json!({ "property": "tags", "method": set_method(query.tag_mode), "value": query.tags }),
        );
    }
    if !query.extra_folder_ids.is_empty() {
        rules.push(json!({
            "property": "folders",
            "method": set_method(query.folder_mode),
            "value": query.extra_folder_ids,
        }));
    }
    if let Some(ext) = &query.ext {
        rules.push(json!({ "property": "type", "method": "equal", "value": ext }));
    }
    if let Some(star) = query.rating {
        let value = if star == 0 {
            "none".to_owned()
        } else {
            star.to_string()
        };
        rules.push(json!({ "property": "rating", "method": "equal", "value": value }));
    }
    if let Some(shape) = &query.shape {
        rules.push(json!({ "property": "shape", "method": "equal", "value": shape }));
    }
    if rules.is_empty() {
        return Err(SourceError::Invalid(
            "Open a folder or add a filter first: a smart folder needs at least one rule.".into(),
        ));
    }
    Ok((
        parent,
        json!([{ "match": "AND", "boolean": "TRUE", "rules": rules }]),
    ))
}

/// A smart folder edit. `conditions` is Eagle's array, stored unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum SmartFolderChange {
    /// A new smart folder after the last child of `parent` (`None`: the top level).
    Create {
        name: String,
        parent: Option<String>,
        conditions: Value,
    },
    /// The new name and rules; the place in the tree stays.
    Update {
        id: String,
        name: String,
        conditions: Value,
    },
}

impl SmartFolderChange {
    /// Checks the name and the rules before anything is written.
    pub fn validate(&self) -> Result<SmartRules, SourceError> {
        let (Self::Create {
            name, conditions, ..
        }
        | Self::Update {
            name, conditions, ..
        }) = self;
        if name.trim().is_empty() {
            return Err(SourceError::Invalid("A smart folder needs a name.".into()));
        }
        if name.encode_utf16().count() > MAX_TEXT_UTF16 {
            return Err(SourceError::Invalid("The name is too long.".into()));
        }
        let rules = SmartRules::parse(conditions)?;
        rules.check_limits()?;
        Ok(rules)
    }
}

#[cfg(test)]
mod tests;

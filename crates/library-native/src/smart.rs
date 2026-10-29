//! Smart folders of the native library: their own tree, writes, and the items a smart folder
//! shows (docs/spec.md §3 "Smart folders", docs/architecture.md §16). The rules are Eagle's JSON,
//! stored unchanged; `library_core::SmartRules` evaluates them.

use std::collections::HashMap;

use library_core::{
    ExtensionCount, ItemLite, RuleItem, Scope, SmartFolder, SmartFolderChange, SmartRules,
    SourceError, order_key_between,
};
use rusqlite::{OptionalExtension, named_params, params};
use serde_json::Value;
use ulid::Ulid;

use crate::error::{Context, Result};
use crate::folders::log;
use crate::library::Library;
use crate::locks::VISIBLE;
use crate::time::now_ms;

/// Deeper than any tree a person builds; stops a damaged parent chain from looping.
const MAX_DEPTH: usize = 64;

/// `id, annotation, url, encrypted, sealed`.
type TextRow = (String, String, String, bool, Option<Vec<u8>>);

struct Row {
    id: String,
    parent: Option<String>,
    name: String,
    conditions: String,
    order_by: Option<String>,
    sort_ascending: Option<bool>,
}

impl Library {
    /// The smart folder tree in sibling order, each with the rules it cannot evaluate.
    pub fn smart_folders(&self) -> Result<Vec<SmartFolder>> {
        let conn = self.conn();
        let mut statement = conn
            .prepare(
                "SELECT id, parent_id, name, conditions, order_by, sort_ascending
                 FROM smart_folder ORDER BY sort_key, id",
            )
            .context("read smart folders")?;
        let rows: Vec<Row> = statement
            .query_map([], |row| {
                Ok(Row {
                    id: row.get(0)?,
                    parent: row.get(1)?,
                    name: row.get(2)?,
                    conditions: row.get(3)?,
                    order_by: row.get(4)?,
                    sort_ascending: row.get(5)?,
                })
            })
            .and_then(Iterator::collect)
            .context("read smart folders")?;
        let mut children: HashMap<Option<&str>, Vec<&Row>> = HashMap::new();
        for row in &rows {
            children.entry(row.parent.as_deref()).or_default().push(row);
        }
        fn build(
            parent: Option<&str>,
            children: &HashMap<Option<&str>, Vec<&Row>>,
            depth: usize,
        ) -> Vec<SmartFolder> {
            if depth > MAX_DEPTH {
                return Vec::new();
            }
            children
                .get(&parent)
                .into_iter()
                .flatten()
                .map(|row| {
                    let conditions: Value =
                        serde_json::from_str(&row.conditions).unwrap_or(Value::Null);
                    let unsupported = match SmartRules::parse(&conditions) {
                        Ok(rules) => rules.unsupported(),
                        Err(_) => vec!["unreadable rules".to_owned()],
                    };
                    SmartFolder {
                        id: row.id.clone(),
                        name: row.name.clone(),
                        children: build(Some(&row.id), children, depth + 1),
                        conditions,
                        order_by: row.order_by.clone(),
                        sort_ascending: row.sort_ascending,
                        unsupported,
                    }
                })
                .collect()
        }
        Ok(build(None, &children, 0))
    }

    /// Creates a smart folder after the last child of its parent, or changes a name and rules.
    /// Returns its id.
    pub fn save_smart_folder(&self, change: &SmartFolderChange) -> Result<String> {
        self.check_writable()?;
        change.validate()?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("save the smart folder")?;
        let id = match change {
            SmartFolderChange::Create {
                name,
                parent,
                conditions,
            } => {
                if let Some(parent) = parent {
                    let known: Option<i64> = tx
                        .query_row(
                            "SELECT 1 FROM smart_folder WHERE id = ?1",
                            params![parent],
                            |row| row.get(0),
                        )
                        .optional()
                        .context("find the smart folder")?;
                    if known.is_none() {
                        return Err(SourceError::Invalid(
                            "The parent smart folder no longer exists.".into(),
                        ));
                    }
                }
                let last: Option<String> = tx
                    .query_row(
                        "SELECT sort_key FROM smart_folder WHERE parent_id IS ?1
                         ORDER BY sort_key DESC LIMIT 1",
                        params![parent],
                        |row| row.get(0),
                    )
                    .optional()
                    .context("read the smart folder order")?;
                let id = Ulid::generate().to_string();
                tx.execute(
                    "INSERT INTO smart_folder (id, parent_id, name, conditions, sort_key)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        id,
                        parent,
                        name.trim(),
                        conditions.to_string(),
                        order_key_between(last.as_deref(), None)
                    ],
                )
                .context("create the smart folder")?;
                log(&tx, "smart_folder", &id, "insert", now)?;
                id
            }
            SmartFolderChange::Update {
                id,
                name,
                conditions,
            } => {
                let changed = tx
                    .execute(
                        "UPDATE smart_folder SET name = ?2, conditions = ?3 WHERE id = ?1",
                        params![id, name.trim(), conditions.to_string()],
                    )
                    .context("edit the smart folder")?;
                if changed == 0 {
                    return Err(SourceError::NotFound);
                }
                log(&tx, "smart_folder", id, "update", now)?;
                id.clone()
            }
        };
        tx.commit().context("save the smart folder")?;
        Ok(id)
    }

    /// Deletes a smart folder and the smart folders under it; items stay as they are.
    pub fn delete_smart_folder(&self, id: &str) -> Result<()> {
        self.check_writable()?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("delete the smart folder")?;
        let deleted = tx
            .execute("DELETE FROM smart_folder WHERE id = ?1", params![id])
            .context("delete the smart folder")?;
        if deleted == 0 {
            return Err(SourceError::NotFound);
        }
        log(&tx, "smart_folder", id, "delete", now)?;
        tx.commit().context("delete the smart folder")
    }

    /// The items of a smart folder: outside the trash, visible, and matching its rules and the
    /// rules of every smart folder above it (docs/architecture.md §16.1).
    pub(crate) fn load_smart(&self, id: &str) -> Result<Vec<ItemLite>> {
        let chain = self.rule_chain(id)?;
        let items = self.load_scope(&Scope::All)?;
        let texts = self.item_texts()?;
        let now = now_ms();
        Ok(items
            .into_iter()
            .filter(|item| {
                let (annotation, url) =
                    texts.get(&item.id.0).map_or(("", ""), |(annotation, url)| {
                        (annotation.as_str(), url.as_str())
                    });
                let rule_item = RuleItem {
                    item,
                    annotation,
                    url,
                };
                chain.iter().all(|rules| rules.matches(&rule_item, now))
            })
            .collect())
    }

    /// The rules of `id` and of each smart folder above it. Unreadable rules match nothing.
    fn rule_chain(&self, id: &str) -> Result<Vec<SmartRules>> {
        let conn = self.conn();
        let mut chain = Vec::new();
        let mut next = Some(id.to_owned());
        while let Some(at) = next.take() {
            if chain.len() > MAX_DEPTH {
                break;
            }
            let row: Option<(String, Option<String>)> = conn
                .query_row(
                    "SELECT conditions, parent_id FROM smart_folder WHERE id = ?1",
                    params![at],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .context("read the smart folder")?;
            let Some((conditions, parent)) = row else {
                if chain.is_empty() {
                    return Err(SourceError::NotFound);
                }
                break;
            };
            let conditions: Value = serde_json::from_str(&conditions).unwrap_or(Value::Null);
            chain.push(SmartRules::parse(&conditions).unwrap_or_else(|_| SmartRules::nothing()));
            next = parent;
        }
        Ok(chain)
    }

    /// The notes and links of visible items outside the trash, opened for unlocked items.
    fn item_texts(&self) -> Result<HashMap<String, (String, String)>> {
        let conn = self.conn();
        let visibility = self.visibility(&conn)?;
        let mut texts = HashMap::new();
        let mut statement = conn
            .prepare(&format!(
                "SELECT id, annotation, url, encrypted, sealed FROM item
                 WHERE deleted_at IS NULL AND (encrypted = 1 OR annotation <> '' OR url <> '')
                 AND {VISIBLE}"
            ))
            .context("read notes and links")?;
        let rows: Vec<TextRow> = statement
            .query_map(
                named_params! {
                    ":hidden": visibility.hidden,
                    ":unlocked": visibility.unlocked,
                },
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .and_then(Iterator::collect)
            .context("read notes and links")?;
        for (id, annotation, url, encrypted, sealed) in rows {
            if encrypted {
                match self.open_fields(&conn, &id, &sealed.unwrap_or_default()) {
                    Ok(fields) => {
                        texts.insert(id, (fields.annotation, fields.url));
                    }
                    Err(error) => tracing::warn!("cannot open an encrypted item: {error}"),
                }
            } else {
                texts.insert(id, (annotation, url));
            }
        }
        Ok(texts)
    }

    /// The extensions of visible items outside the trash, the most common first.
    pub fn extensions(&self) -> Result<Vec<ExtensionCount>> {
        let conn = self.conn();
        let visibility = self.visibility(&conn)?;
        let mut statement = conn
            .prepare(&format!(
                "SELECT ext, count(*) FROM item WHERE deleted_at IS NULL AND {VISIBLE}
                 GROUP BY ext ORDER BY count(*) DESC, ext"
            ))
            .context("count extensions")?;
        statement
            .query_map(
                named_params! {
                    ":hidden": visibility.hidden,
                    ":unlocked": visibility.unlocked,
                },
                |row| {
                    Ok(ExtensionCount {
                        ext: row.get(0)?,
                        count: row.get(1)?,
                    })
                },
            )
            .and_then(Iterator::collect)
            .context("count extensions")
    }
}

//! The search field in the native library (docs/spec.md §5 "Search", D-047): every term is a
//! case-insensitive substring of an item's name, note, link, tags or folder names, and the
//! terms combine as `library_core::SearchExpr` says.
//!
//! Names, notes and links come from the trigram index for terms of three or more characters; a
//! shorter term reads the rows instead. Encrypted items have no index row: an item of an
//! unlocked folder is matched on its opened fields, a hidden one not at all. Tags and folder
//! names are matched on their own tables. An open library always has the index: opening migrates
//! an older one, or refuses it read-only.

use std::collections::{HashMap, HashSet};

use library_core::{ItemId, SearchMatch, fold, parse_search};
use rusqlite::types::FromSql;
use rusqlite::{Connection, ToSql, named_params, params};

use crate::error::{Context, Result};
use crate::library::Library;
use crate::locks::VISIBLE;

/// The trigram index matches from three characters.
const INDEXED_FROM: usize = 3;
/// An item's own texts, folded, for terms the index cannot answer.
struct Row {
    id: String,
    text: String,
}

impl Library {
    /// The items `text` keeps; a text without terms keeps every item.
    pub fn search(&self, text: &str) -> Result<SearchMatch> {
        let Some(expr) = parse_search(text) else {
            return Ok(SearchMatch::Except(HashSet::new()));
        };
        let terms = expr.terms();
        let conn = self.conn();
        let indexed = |term: &str| term.chars().count() >= INDEXED_FROM;
        let plain = if terms.iter().all(|term| indexed(term)) {
            Vec::new()
        } else {
            plain_rows(&conn)?
        };
        let sealed = self.sealed_rows(&conn)?;
        let tags: Vec<(i64, String)> = named(&conn, "SELECT id, name FROM tag")?;
        let folders: Vec<(String, String)> = named(&conn, "SELECT id, name FROM folder")?;

        let mut matches: HashMap<String, HashSet<ItemId>> = HashMap::new();
        for term in terms {
            let needle = fold(term);
            let mut ids: HashSet<ItemId> = HashSet::new();
            if indexed(term) {
                let mut statement = conn
                    .prepare_cached("SELECT item.id FROM item_fts JOIN item ON item.seq = item_fts.rowid WHERE item_fts MATCH ?1")
                    .context("search")?;
                let found = statement
                    .query_map(params![phrase(term)], |row| row.get::<_, String>(0))
                    .context("search")?;
                for id in found {
                    ids.insert(ItemId(id.context("search")?));
                }
            } else {
                ids.extend(
                    plain
                        .iter()
                        .filter(|row| row.text.contains(&needle))
                        .map(|row| ItemId(row.id.clone())),
                );
            }
            ids.extend(
                sealed
                    .iter()
                    .filter(|row| row.text.contains(&needle))
                    .map(|row| ItemId(row.id.clone())),
            );
            for (tag, _) in tags.iter().filter(|(_, name)| name.contains(&needle)) {
                linked(
                    &conn,
                    "SELECT item_id FROM item_tag WHERE tag_id = ?1",
                    tag,
                    &mut ids,
                )?;
            }
            for (folder, _) in folders.iter().filter(|(_, name)| name.contains(&needle)) {
                linked(
                    &conn,
                    "SELECT item_id FROM item_folder WHERE folder_id = ?1",
                    folder,
                    &mut ids,
                )?;
            }
            matches.insert(term.to_owned(), ids);
        }
        Ok(expr.evaluate(&matches))
    }

    /// Encrypted items the user may see, with their opened texts.
    fn sealed_rows(&self, conn: &Connection) -> Result<Vec<Row>> {
        let visibility = self.visibility(conn)?;
        let mut statement = conn
            .prepare(&format!(
                "SELECT id, sealed FROM item WHERE encrypted = 1 AND {VISIBLE}"
            ))
            .context("search encrypted items")?;
        let found: Vec<(String, Option<Vec<u8>>)> = statement
            .query_map(
                named_params! {
                    ":hidden": visibility.hidden,
                    ":unlocked": visibility.unlocked,
                },
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .and_then(Iterator::collect)
            .context("search encrypted items")?;
        let mut rows = Vec::with_capacity(found.len());
        for (id, sealed) in found {
            match self.open_fields(conn, &id, &sealed.unwrap_or_default()) {
                Ok(fields) => rows.push(Row {
                    text: fold(&format!(
                        "{}\n{}\n{}",
                        fields.name, fields.annotation, fields.url
                    )),
                    id,
                }),
                Err(error) => tracing::warn!("cannot open an encrypted item: {error}"),
            }
        }
        Ok(rows)
    }
}

/// A term as one FTS5 phrase: with the trigram tokenizer, a substring.
fn phrase(term: &str) -> String {
    format!("\"{}\"", term.replace('"', "\"\""))
}

fn plain_rows(conn: &Connection) -> Result<Vec<Row>> {
    let mut statement = conn
        .prepare("SELECT id, name, annotation, url FROM item WHERE encrypted = 0")
        .context("search")?;
    statement
        .query_map([], |row| {
            Ok(Row {
                id: row.get(0)?,
                text: fold(&format!(
                    "{}\n{}\n{}",
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?
                )),
            })
        })
        .and_then(Iterator::collect)
        .context("search")
}

/// `(id, folded name)` of every tag or folder.
fn named<T: FromSql>(conn: &Connection, sql: &str) -> Result<Vec<(T, String)>> {
    let mut statement = conn.prepare(sql).context("search names")?;
    statement
        .query_map([], |row| Ok((row.get(0)?, fold(&row.get::<_, String>(1)?))))
        .and_then(Iterator::collect)
        .context("search names")
}

fn linked(
    conn: &Connection,
    sql: &str,
    owner: &dyn ToSql,
    ids: &mut HashSet<ItemId>,
) -> Result<()> {
    let mut statement = conn.prepare_cached(sql).context("search links")?;
    let found = statement
        .query_map([owner], |row| row.get::<_, String>(0))
        .context("search links")?;
    for id in found {
        ids.insert(ItemId(id.context("search links")?));
    }
    Ok(())
}

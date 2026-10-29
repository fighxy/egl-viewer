//! The search field's syntax (docs/spec.md §5 "Search"): words are AND-ed, `OR` between words,
//! parentheses group, a leading `-` excludes a word or a group, and `"…"` keeps spaces inside one
//! term. Sources match single terms (`LibrarySource::search_ids`); this module parses the text
//! and combines the matches.

use std::collections::{HashMap, HashSet};

use crate::model::ItemId;

/// A parsed search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchExpr {
    /// Items whose text contains the term, ignoring case.
    Term(String),
    And(Vec<SearchExpr>),
    Or(Vec<SearchExpr>),
    Not(Box<SearchExpr>),
}

/// The items a search keeps: a set, or everything except a set (a search that only excludes).
/// Membership needs no list of every item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchMatch {
    Only(HashSet<ItemId>),
    Except(HashSet<ItemId>),
}

impl SearchMatch {
    pub fn contains(&self, id: &ItemId) -> bool {
        match self {
            Self::Only(ids) => ids.contains(id),
            Self::Except(ids) => !ids.contains(id),
        }
    }

    fn not(self) -> Self {
        match self {
            Self::Only(ids) => Self::Except(ids),
            Self::Except(ids) => Self::Only(ids),
        }
    }

    fn and(self, other: Self) -> Self {
        use SearchMatch::{Except, Only};
        match (self, other) {
            (Only(a), Only(b)) => Only(a.intersection(&b).cloned().collect()),
            (Only(a), Except(b)) | (Except(b), Only(a)) => {
                Only(a.difference(&b).cloned().collect())
            }
            (Except(a), Except(b)) => Except(a.union(&b).cloned().collect()),
        }
    }

    fn or(self, other: Self) -> Self {
        use SearchMatch::{Except, Only};
        match (self, other) {
            (Only(a), Only(b)) => Only(a.union(&b).cloned().collect()),
            (Only(a), Except(b)) | (Except(b), Only(a)) => {
                Except(b.difference(&a).cloned().collect())
            }
            (Except(a), Except(b)) => Except(a.intersection(&b).cloned().collect()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Open,
    Close,
    Or,
    Not,
    Word(String),
}

fn tokens(text: &str) -> Vec<Token> {
    let mut found = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&next) = chars.peek() {
        match next {
            c if c.is_whitespace() => {
                chars.next();
            }
            '(' => {
                chars.next();
                found.push(Token::Open);
            }
            ')' => {
                chars.next();
                found.push(Token::Close);
            }
            '"' => {
                chars.next();
                let phrase: String = chars.by_ref().take_while(|&c| c != '"').collect();
                if !phrase.trim().is_empty() {
                    found.push(Token::Word(phrase.trim().to_owned()));
                }
            }
            _ => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    // A quote right after a leading `-` starts the excluded phrase.
                    if c.is_whitespace() || c == '(' || c == ')' || (c == '"' && word == "-") {
                        break;
                    }
                    word.push(c);
                    chars.next();
                }
                // A leading `-` excludes what follows: a word, a phrase or a group.
                if let Some(rest) = word.strip_prefix('-') {
                    found.push(Token::Not);
                    if !rest.is_empty() {
                        found.push(Token::Word(rest.to_owned()));
                    }
                } else if word == "OR" {
                    found.push(Token::Or);
                } else {
                    found.push(Token::Word(word));
                }
            }
        }
    }
    found
}

/// Parses the search text; `None` when it holds no term. Never fails: stray parentheses, a
/// dangling `OR` or `-` are left out, and a missing `)` closes at the end. `OR` is an operator
/// only in capitals; `or` is a word.
pub fn parse_search(text: &str) -> Option<SearchExpr> {
    let tokens = tokens(text);
    let mut at = 0;
    let mut parsed = Vec::new();
    // Unmatched `)` at the top level are skipped.
    while at < tokens.len() {
        if let Some(expr) = parse_or(&tokens, &mut at) {
            parsed.push(expr);
        }
        if tokens.get(at) == Some(&Token::Close) {
            at += 1;
        }
    }
    join(parsed, SearchExpr::And)
}

fn parse_or(tokens: &[Token], at: &mut usize) -> Option<SearchExpr> {
    let mut branches = Vec::new();
    loop {
        if let Some(branch) = parse_and(tokens, at) {
            branches.push(branch);
        }
        if tokens.get(*at) == Some(&Token::Or) {
            *at += 1;
        } else {
            break;
        }
    }
    join(branches, SearchExpr::Or)
}

fn parse_and(tokens: &[Token], at: &mut usize) -> Option<SearchExpr> {
    let mut parts = Vec::new();
    while let Some(token) = tokens.get(*at) {
        if matches!(token, Token::Or | Token::Close) {
            break;
        }
        if let Some(part) = parse_unary(tokens, at) {
            parts.push(part);
        }
    }
    join(parts, SearchExpr::And)
}

fn parse_unary(tokens: &[Token], at: &mut usize) -> Option<SearchExpr> {
    match tokens.get(*at)? {
        Token::Not => {
            *at += 1;
            match tokens.get(*at) {
                Some(Token::Word(_) | Token::Open | Token::Not) => {
                    parse_unary(tokens, at).map(|inner| SearchExpr::Not(Box::new(inner)))
                }
                _ => None,
            }
        }
        Token::Open => {
            *at += 1;
            let inner = parse_or(tokens, at);
            if tokens.get(*at) == Some(&Token::Close) {
                *at += 1;
            }
            inner
        }
        Token::Word(word) => {
            *at += 1;
            Some(SearchExpr::Term(word.clone()))
        }
        Token::Or | Token::Close => None,
    }
}

fn join(
    mut parts: Vec<SearchExpr>,
    combine: fn(Vec<SearchExpr>) -> SearchExpr,
) -> Option<SearchExpr> {
    match parts.len() {
        0 => None,
        1 => parts.pop(),
        _ => Some(combine(parts)),
    }
}

impl SearchExpr {
    /// Every distinct term, in order of appearance.
    pub fn terms(&self) -> Vec<&str> {
        let mut found: Vec<&str> = Vec::new();
        self.collect_terms(&mut found);
        found
    }

    fn collect_terms<'a>(&'a self, found: &mut Vec<&'a str>) {
        match self {
            Self::Term(term) => {
                if !found.contains(&term.as_str()) {
                    found.push(term);
                }
            }
            Self::And(parts) | Self::Or(parts) => {
                for part in parts {
                    part.collect_terms(found);
                }
            }
            Self::Not(inner) => inner.collect_terms(found),
        }
    }

    /// Combines the items each term matches (`matches[term]`; a missing term matches nothing).
    pub fn evaluate(&self, matches: &HashMap<String, HashSet<ItemId>>) -> SearchMatch {
        match self {
            Self::Term(term) => SearchMatch::Only(matches.get(term).cloned().unwrap_or_default()),
            Self::Not(inner) => inner.evaluate(matches).not(),
            Self::And(parts) => parts
                .iter()
                .map(|part| part.evaluate(matches))
                .reduce(SearchMatch::and)
                .unwrap_or(SearchMatch::Except(HashSet::new())),
            Self::Or(parts) => parts
                .iter()
                .map(|part| part.evaluate(matches))
                .reduce(SearchMatch::or)
                .unwrap_or(SearchMatch::Only(HashSet::new())),
        }
    }
}

/// The form both sides of a comparison take before a substring test: Unicode lowercase, so
/// "ПЛАКАТ" contains "плак" (tags keep their case; only the search ignores it).
pub fn fold(text: &str) -> String {
    text.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use SearchExpr::{And, Not, Or, Term};

    fn term(text: &str) -> SearchExpr {
        Term(text.into())
    }

    #[test]
    fn words_are_anded_and_or_binds_looser() {
        assert_eq!(
            parse_search("poster swiss"),
            Some(And(vec![term("poster"), term("swiss")]))
        );
        assert_eq!(
            parse_search("a b OR c"),
            Some(Or(vec![And(vec![term("a"), term("b")]), term("c")]))
        );
        assert_eq!(
            parse_search("a (b OR c)"),
            Some(And(vec![term("a"), Or(vec![term("b"), term("c")])]))
        );
        // Lowercase `or` is a word.
        assert_eq!(
            parse_search("a or b"),
            Some(And(vec![term("a"), term("or"), term("b")]))
        );
    }

    #[test]
    fn a_leading_minus_excludes_a_word_or_a_group() {
        assert_eq!(parse_search("-draft"), Some(Not(Box::new(term("draft")))));
        assert_eq!(
            parse_search("poster -(a OR b)"),
            Some(And(vec![
                term("poster"),
                Not(Box::new(Or(vec![term("a"), term("b")])))
            ]))
        );
        // Inside a word the dash is a character.
        assert_eq!(parse_search("poster-final"), Some(term("poster-final")));
        assert_eq!(
            parse_search("\"swiss style\" -\"old\""),
            Some(And(vec![term("swiss style"), Not(Box::new(term("old")))]))
        );
    }

    #[test]
    fn broken_syntax_is_forgiven() {
        assert_eq!(
            parse_search("(a OR b"),
            Some(Or(vec![term("a"), term("b")]))
        );
        assert_eq!(parse_search("a) b"), Some(And(vec![term("a"), term("b")])));
        assert_eq!(parse_search("OR a OR"), Some(term("a")));
        assert_eq!(parse_search("a -"), Some(term("a")));
        assert_eq!(parse_search("  ( ) - OR "), None);
        assert_eq!(parse_search("плакат"), Some(term("плакат")));
    }

    #[test]
    fn terms_are_listed_once() {
        let parsed = parse_search("a (b OR a) -c").unwrap();
        assert_eq!(parsed.terms(), ["a", "b", "c"]);
    }

    fn ids(names: &[&str]) -> HashSet<ItemId> {
        names.iter().map(|name| ItemId((*name).into())).collect()
    }

    #[test]
    fn exclusions_need_no_list_of_every_item() {
        let matches = HashMap::from([
            ("a".to_owned(), ids(&["1", "2"])),
            ("b".to_owned(), ids(&["2", "3"])),
        ]);
        let eval = |text: &str| parse_search(text).unwrap().evaluate(&matches);
        assert_eq!(eval("a b"), SearchMatch::Only(ids(&["2"])));
        assert_eq!(eval("a OR b"), SearchMatch::Only(ids(&["1", "2", "3"])));
        assert_eq!(eval("a -b"), SearchMatch::Only(ids(&["1"])));
        assert_eq!(eval("-a"), SearchMatch::Except(ids(&["1", "2"])));
        assert_eq!(eval("-a -b"), SearchMatch::Except(ids(&["1", "2", "3"])));
        assert_eq!(eval("-a OR b"), SearchMatch::Except(ids(&["1"])));
        assert_eq!(eval("-(a b)"), SearchMatch::Except(ids(&["2"])));
        assert_eq!(eval("unknown"), SearchMatch::Only(HashSet::new()));
        let except = eval("-a");
        assert!(except.contains(&ItemId("9".into())));
        assert!(!except.contains(&ItemId("1".into())));
    }

    #[test]
    fn folding_handles_cyrillic() {
        assert_eq!(fold("ПЛАКАТ Swiss"), "плакат swiss");
    }
}

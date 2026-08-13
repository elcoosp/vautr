//! FTS5 query preparation (VTR-055).
//!
//! Rewrites a raw user query into a safe SQLite FTS5 `MATCH` expression and
//! surfaces signals the UI needs:
//! - stopword stripping (common words add no search value),
//! - prefix vs exact vs substring (leading-wildcard) handling,
//! - a `leading_wildcard` flag so the UI can warn that FTS5 cannot use its
//!   index for a leading `*` and will do a full scan (TDD3).
//!
//! The FTS5 table uses the `unicode61` tokenizer, which is already
//! case-insensitive and diacritic-insensitive, so rewriting only needs to
//! normalise wildcards and drop stopwords (db-contract §4).

/// Result of preparing a user query for FTS5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSearch {
    /// The `MATCH` expression to bind (empty when the query is effectively
    /// empty — callers should fall back to `recent_overviews`).
    pub match_expr: String,
    /// True when the query contains a leading (or internal) wildcard that
    /// defeats the FTS5 index — the UI should warn about a slow scan.
    pub leading_wildcard: bool,
    /// True when there is nothing searchable left after rewriting.
    pub is_empty: bool,
}

/// Common English stopwords removed from searches (they match almost every
/// row and only add noise / slow the query).
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "is", "it", "with", "by", "at",
    "from", "as", "be", "that", "this", "was", "are",
];

/// Prepare a raw search string for FTS5.
///
/// Rules:
/// - Whitespace-split into tokens; FTS5 ANDs them.
/// - Whole-word stopwords (without a wildcard) are dropped.
/// - Each surviving token is double-quoted to neutralise FTS5 syntax injection;
///   a trailing `*` is preserved as a prefix operator (`"stem"*`).
/// - A token that begins with `*` (or has a `*` not at the end) sets
///   `leading_wildcard` (TDD3) — e.g. `*ank` or `a*nk`.
pub fn prepare_search(raw: &str) -> PreparedSearch {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return PreparedSearch {
            match_expr: String::new(),
            leading_wildcard: false,
            is_empty: true,
        };
    }

    let mut tokens = Vec::new();
    let mut leading_wildcard = false;

    for token in trimmed.split_whitespace() {
        // Detect a leading/internal wildcard before stripping it for the
        // stopword check (a wildcarded stopword like "*the" is still a scan).
        if token.starts_with('*') || (token.contains('*') && !token.ends_with('*')) {
            leading_wildcard = true;
        }

        // Strip wildcard chars to test against the stopword list.
        let bare = token.replace('*', "").to_lowercase();
        if bare.is_empty() {
            continue;
        }
        if bare.as_str().len() <= 2 && token.contains('*') {
            // A 1-2 char stem with a wildcard is useless noise; skip it but it
            // was already flagged as a scan above if leading.
            continue;
        }
        if STOPWORDS.contains(&bare.as_str()) && !token.contains('*') {
            continue;
        }

        // Quote the stem and re-attach a trailing prefix operator.
        let stem = bare.replace(['"', '\\', '\''], "");
        let expr = if token.ends_with('*') {
            format!("\"{stem}\"*")
        } else {
            format!("\"{stem}\"")
        };
        tokens.push(expr);
    }

    if tokens.is_empty() {
        return PreparedSearch {
            match_expr: String::new(),
            leading_wildcard,
            is_empty: true,
        };
    }

    PreparedSearch {
        match_expr: tokens.join(" "),
        leading_wildcard,
        is_empty: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_is_empty() {
        let p = prepare_search("   ");
        assert!(p.is_empty);
        assert!(p.match_expr.is_empty());
    }

    #[test]
    fn strips_stopwords() {
        let p = prepare_search("the bank of");
        assert_eq!(p.match_expr, "\"bank\"");
        assert!(!p.is_empty);
    }

    #[test]
    fn prefix_wildcard_preserved() {
        let p = prepare_search("ban");
        // A bare term is an exact token match unless the caller adds `*`.
        assert_eq!(p.match_expr, "\"ban\"");
        let p2 = prepare_search("ban*");
        assert_eq!(p2.match_expr, "\"ban\"*");
    }

    #[test]
    fn leading_wildcard_flagged() {
        let p = prepare_search("*ank");
        assert!(p.leading_wildcard);
        // FTS5 cannot express a leading wildcard, so the `*` is stripped and the
        // term is matched as a token; the flag tells the UI to warn about the scan.
        assert_eq!(p.match_expr, "\"ank\"");
    }

    #[test]
    fn internal_wildcard_flagged() {
        assert!(prepare_search("a*nk").leading_wildcard);
    }

    #[test]
    fn quotes_special_chars() {
        // A token with FTS5 syntax must still come out quoted/safe.
        let p = prepare_search("\"foo\"bar");
        assert_eq!(p.match_expr, "\"foobar\"");
    }
}

//! Plugin Store full-text search helpers (server + SPA parity).
//!
//! Fields searched: name, id, description, category, author, keywords.
//!
//! Short tokens (1 to 2 chars, e.g. `ai`) use word/token matching so hits like
//! Em**ai**l / Analyt**ics** are reduced, while whole words such as `AI` in a
//! description or keyword still match. Longer queries keep case-insensitive
//! substring matching across the same fields.

use crate::plugins::CatalogEntry;

/// Queries this short use token (word-boundary) matching instead of substring.
pub(crate) const SHORT_TOKEN_MAX: usize = 2;

pub(crate) fn store_search_blob(entry: &CatalogEntry) -> String {
    let mut parts = vec![
        entry.name.as_str(),
        entry.id.as_str(),
        entry.description.as_str(),
        entry.category.as_str(),
        entry.author.as_str(),
    ];
    for kw in &entry.keywords {
        parts.push(kw.as_str());
    }
    parts.join(" ")
}

fn tokenize(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            cur.push(ch.to_ascii_lowercase());
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn field_matches(field: &str, q: &str) -> bool {
    let field_l = field.to_ascii_lowercase();
    if q.chars().count() <= SHORT_TOKEN_MAX {
        tokenize(&field_l).iter().any(|t| t == q)
    } else {
        field_l.contains(q)
    }
}

fn blob_matches(blob: &str, q: &str) -> bool {
    let blob_l = blob.to_ascii_lowercase();
    if q.chars().count() <= SHORT_TOKEN_MAX {
        tokenize(&blob_l).iter().any(|t| t == q)
    } else {
        blob_l.contains(q)
    }
}

/// Higher is better. `None` means no match.
pub(crate) fn store_match_score(entry: &CatalogEntry, query: &str) -> Option<u32> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Some(0);
    }
    let mut best = 0u32;
    let mut hit = false;
    if field_matches(&entry.name, &q) {
        hit = true;
        best = best.max(100);
    }
    if field_matches(&entry.id, &q) {
        hit = true;
        best = best.max(90);
    }
    for kw in &entry.keywords {
        if field_matches(kw, &q) {
            hit = true;
            best = best.max(85);
        }
    }
    if field_matches(&entry.category, &q) {
        hit = true;
        best = best.max(70);
    }
    if field_matches(&entry.author, &q) {
        hit = true;
        best = best.max(60);
    }
    if field_matches(&entry.description, &q) {
        hit = true;
        best = best.max(75);
    }
    // Multi-word / leftover: whole-blob check (covers spaced phrases).
    if !hit && blob_matches(&store_search_blob(entry), &q) {
        hit = true;
        best = best.max(50);
    }
    if hit { Some(best) } else { None }
}

pub(crate) fn entry_matches_store_query(entry: &CatalogEntry, query: &str) -> bool {
    store_match_score(entry, query).is_some()
}

/// Host-package haystack match using the same short-token rules.
pub(crate) fn host_text_matches(
    name: &str,
    id: &str,
    description: &str,
    category: &str,
    query: &str,
) -> bool {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return true;
    }
    field_matches(name, &q)
        || field_matches(id, &q)
        || field_matches(description, &q)
        || field_matches(category, &q)
}

/// JavaScript helpers shared with the Store SPA (keep behavior aligned).
pub(crate) fn store_search_js_helpers() -> &'static str {
    r#"
  var CPN_SHORT_TOKEN_MAX = 2;
  function cpnStoreTokens(s) {
    return String(s || '').toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);
  }
  function cpnFieldMatches(field, q) {
    var f = String(field || '').toLowerCase();
    if (!q) return true;
    if (q.length <= CPN_SHORT_TOKEN_MAX) {
      return cpnStoreTokens(f).indexOf(q) >= 0;
    }
    return f.indexOf(q) >= 0;
  }
  function cpnHayMatches(hay, q) {
    return cpnFieldMatches(hay, q);
  }
  function cpnMatchScore(hay, q) {
    if (!q) return 0;
    if (!cpnHayMatches(hay, q)) return -1;
    var tokens = cpnStoreTokens(hay);
    if (q.length <= CPN_SHORT_TOKEN_MAX) {
      if (tokens[0] === q) return 100;
      if (tokens.indexOf(q) >= 0) return 80;
      return 50;
    }
    var low = String(hay || '').toLowerCase();
    if (low.indexOf(q) === 0) return 100;
    return 60;
  }
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, name: &str, description: &str, keywords: &[&str]) -> CatalogEntry {
        CatalogEntry {
            id: id.into(),
            name: name.into(),
            category: "Utility".into(),
            version: "1.0.0".into(),
            description: description.into(),
            author: "master3395".into(),
            pricing: "free".into(),
            released_on: String::new(),
            updated_on: String::new(),
            install_count: 0,
            featured: false,
            uninstall_impacts: vec![],
            host_scoped: false,
            keywords: keywords.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn short_ai_matches_description_word_not_email_substring() {
        let agent = entry(
            "mrAgent",
            "Mr Agent",
            "AI chat assistant for CPN with OpenAI keys.",
            &["ai", "mcp", "assistant"],
        );
        let bimi = entry("bimi", "BIMI", "Unlocks Email -> BIMI in CPN Panel.", &[]);
        assert!(entry_matches_store_query(&agent, "ai"));
        assert!(!entry_matches_store_query(&bimi, "ai"));
        assert!(entry_matches_store_query(&bimi, "email"));
    }

    #[test]
    fn mcp_keyword_and_description_match() {
        let agent = entry(
            "mrAgent",
            "Mr Agent",
            "AI assistant with MCP (Model Context Protocol) tools.",
            &["mcp", "ai"],
        );
        assert!(entry_matches_store_query(&agent, "mcp"));
        assert!(entry_matches_store_query(&agent, "MCP"));
        assert!(store_match_score(&agent, "mcp").unwrap() >= 75);
    }

    #[test]
    fn longer_substring_still_works() {
        let agent = entry("mrAgent", "Mr Agent", "AI chat assistant for CPN.", &[]);
        assert!(entry_matches_store_query(&agent, "assistant"));
        assert!(entry_matches_store_query(&agent, "chat assistant"));
    }

    #[test]
    fn analytics_and_jails_false_positives_dropped_for_ai() {
        let csp = entry(
            "cspManager",
            "CSP Manager",
            "Supports Google Analytics and Tag Manager.",
            &[],
        );
        let fg = entry(
            "fileGator",
            "FileGator",
            "jails the repository to that site home.",
            &[],
        );
        assert!(!entry_matches_store_query(&csp, "ai"));
        assert!(!entry_matches_store_query(&fg, "ai"));
    }
}

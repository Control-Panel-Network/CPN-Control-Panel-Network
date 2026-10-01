//! Log entries as the same `data-table` markup the Overview (Panel activity) uses, so the
//! Activity Board container rules apply: a normal table on wide panels and stacked, labelled
//! cards (Time / Source / Level / Details) on narrow ones, never a horizontal page scroll.

use crate::panel_server_logs::LogEntry;
use crate::panel_server_logs_fields::{Fields, parse};
use crate::panel_server_logs_kind::HostLogKind;

/// Messages longer than this fold behind a "Show full line" toggle.
const FOLD_CHARS: usize = 240;
const FOLD_PREVIEW_CHARS: usize = 200;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn rows_styles() -> &'static str {
    r#"<style>
.log-list .data-table td[data-label="Time"]{white-space:nowrap;font-variant-numeric:tabular-nums;}
.log-list .data-table td[data-label="Level"]{white-space:nowrap;}
.log-list .data-table td code{white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word;font-size:12px;}
.log-lvl{display:inline-block;padding:1px 9px;border-radius:999px;font-size:12px;font-weight:700;
  border:1px solid transparent;}
.log-lvl.ok{background:#dcfce7;color:#166534;border-color:#86efac;}
.log-lvl.info{background:#dbeafe;color:#1e3a8a;border-color:#93c5fd;}
.log-lvl.warn{background:#fef3c7;color:#92400e;border-color:#fcd34d;}
.log-lvl.err{background:#fee2e2;color:#991b1b;border-color:#fca5a5;}
.log-lvl.debug{background:#e2e8f0;color:#334155;border-color:#cbd5e1;}
.log-scope{display:inline-block;padding:1px 8px;border-radius:999px;background:#e0e7ff;color:#1e3a8a;
  font-size:12px;font-weight:700;max-width:100%;overflow-wrap:anywhere;}
.log-none{color:var(--muted);}
.log-fold summary,.log-raw summary{cursor:pointer;list-style:none;}
.log-fold summary::-webkit-details-marker,.log-raw summary::-webkit-details-marker{display:none;}
.log-fold .more,.log-raw summary{display:inline-block;margin-top:2px;font-weight:700;color:#1d4ed8;font-size:12px;}
.log-fold .more{margin-left:6px;}
.log-fold[open] .more,.log-fold[open] summary code{display:none;}
.log-raw{margin-top:4px;}
.log-raw code{display:block;margin-top:4px;padding:6px 8px;border-radius:8px;background:rgba(100,116,139,.12);}
[data-color-mode="dark"] .log-lvl.ok{background:#14532d;color:#bbf7d0;border-color:#166534;}
[data-color-mode="dark"] .log-lvl.info{background:#1e3a8a;color:#bfdbfe;border-color:#1d4ed8;}
[data-color-mode="dark"] .log-lvl.warn{background:#78350f;color:#fde68a;border-color:#b45309;}
[data-color-mode="dark"] .log-lvl.err{background:#7f1d1d;color:#fecaca;border-color:#b91c1c;}
[data-color-mode="dark"] .log-lvl.debug{background:#334155;color:#e2e8f0;border-color:#475569;}
[data-color-mode="dark"] .log-scope{background:#1e293b;color:#bfdbfe;}
[data-color-mode="dark"] .log-fold .more,[data-color-mode="dark"] .log-raw summary{color:#93c5fd;}
</style>"#
}

/// Column heading for the origin of a line.
pub fn source_heading(kind: HostLogKind) -> &'static str {
    if kind.per_site() {
        "Site / source"
    } else {
        "Source"
    }
}

fn message_html(fields: &Fields, raw: &str) -> String {
    let long = fields.message.chars().count() > FOLD_CHARS;
    let mut out = if long {
        let preview: String = fields.message.chars().take(FOLD_PREVIEW_CHARS).collect();
        format!(
            r#"<details class="log-fold"><summary><code>{}...</code><span class="more">Show full line</span></summary><code>{}</code></details>"#,
            html_escape(&preview),
            html_escape(&fields.message)
        )
    } else {
        format!("<code>{}</code>", html_escape(&fields.message))
    };
    if fields.parsed {
        out.push_str(&format!(
            r#"<details class="log-raw"><summary>Raw line</summary><code>{}</code></details>"#,
            html_escape(raw)
        ));
    }
    out
}

fn row_html(kind: HostLogKind, entry: &LogEntry, with_level: bool) -> String {
    let fields = parse(kind, entry);
    let time = if entry.stamp.is_empty() {
        r#"<span class="log-none">-</span>"#.to_string()
    } else {
        format!("<time>{}</time>", html_escape(&entry.stamp))
    };
    let source = if fields.source.is_empty() {
        r#"<span class="log-none">-</span>"#.to_string()
    } else {
        format!(
            r#"<span class="log-scope">{}</span>"#,
            html_escape(&fields.source)
        )
    };
    let level_cell = if with_level {
        let inner = match &fields.level {
            Some(l) => format!(
                r#"<span class="log-lvl {}">{}</span>"#,
                l.class.css(),
                html_escape(&l.label)
            ),
            None => r#"<span class="log-none">-</span>"#.to_string(),
        };
        format!(r#"<td data-label="Level">{inner}</td>"#)
    } else {
        String::new()
    };
    format!(
        r#"<tr><td data-label="Time">{time}</td><td data-label="{src_head}">{source}</td>{level_cell}<td data-label="Details">{msg}</td></tr>"#,
        src_head = source_heading(kind),
        msg = message_html(&fields, &entry.text),
    )
}

/// Table of one page of entries. The Level column only appears when a line on the page has one.
pub fn rows_table(kind: HostLogKind, entries: &[LogEntry]) -> String {
    let with_level = entries.iter().any(|e| parse(kind, e).level.is_some());
    let level_head = if with_level { "<th>Level</th>" } else { "" };
    let mut table = format!(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Time</th><th>{}</th>{level_head}<th>Details</th></tr></thead><tbody>"#,
        source_heading(kind)
    );
    for entry in entries {
        table.push_str(&row_html(kind, entry, with_level));
    }
    table.push_str("</tbody></table></div>");
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(scope: &str, text: &str) -> LogEntry {
        LogEntry {
            stamp: "01/10/2026 14:05:09".into(),
            scope: scope.into(),
            text: text.into(),
        }
    }

    #[test]
    fn rows_use_the_overview_table_markup_with_labelled_cells() {
        let html = rows_table(
            HostLogKind::Access,
            &[entry(
                "example.com",
                r#"1.1.1.1 - - "GET /a HTTP/1.1" 200 12 "-" "ua""#,
            )],
        );
        assert!(html.contains(r#"<table class="data-table">"#));
        for label in ["Time", "Site / source", "Level", "Details"] {
            assert!(
                html.contains(&format!(r#"data-label="{label}""#)),
                "{label}"
            );
            assert!(html.contains(&format!("<th>{label}</th>")), "{label}");
        }
        assert!(html.contains("01/10/2026 14:05:09"));
        assert!(html.contains(r#"log-scope">example.com"#));
        assert!(html.contains("log-lvl ok"));
        assert!(html.contains("GET /a (HTTP/1.1) from 1.1.1.1, 12 bytes"));
        // The raw line stays reachable.
        assert!(html.contains("Raw line"));
    }

    #[test]
    fn level_column_is_omitted_when_no_line_has_a_level() {
        let html = rows_table(HostLogKind::Panel, &[entry("", "Started panel")]);
        assert!(!html.contains("<th>Level</th>"));
        assert!(!html.contains(r#"data-label="Level""#));
        assert!(html.contains("<th>Source</th>"));
        assert!(!html.contains("Raw line"));
    }

    #[test]
    fn long_messages_fold_and_html_is_escaped() {
        let long = format!("{} <b>x</b>", "y".repeat(400));
        let html = rows_table(HostLogKind::Error, &[entry("server", &long)]);
        assert!(html.contains("Show full line"));
        assert!(html.contains("&lt;b&gt;x&lt;/b&gt;"));
        assert!(!html.contains("<b>x</b>"));
    }
}

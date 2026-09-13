//! Manage Overview tab: disk/bandwidth cards plus host CPU/memory charts.

use crate::panel_ops_db::list_databases;
use crate::panel_ops_ftp::detect_ftp;
use crate::panel_user_prefs::load_user_minimalist_mode;
use crate::panel_website_bandwidth::{BandwidthInfo, bandwidth_for_site};
use crate::panel_website_manage_overview_script::overview_metrics_script;
use crate::panel_website_manage_ui::{html_escape, resource_card, ssl_status_card};
use crate::panel_website_metrics_chart::metrics_chart_svg;
use crate::panel_website_metrics_ring::{
    MetricSample, WINDOW_SECS, load_samples, record_host_sample, stats_for,
};
use crate::panel_website_resources::{approx_dir_bytes, format_bytes};
use crate::service_detect::detect_web_server_label;
use crate::sites::{SiteRecord, is_legacy_docroot, site_home_from_record};
use std::path::Path;

pub fn tab_overview(site: &SiteRecord, username: &str) -> String {
    let minimalist = load_user_minimalist_mode(username);
    let disk_bytes = approx_dir_bytes(Path::new(&site.docroot), 8_000);
    let disk = disk_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "Unavailable".into());
    let disk_pct = disk_bytes
        .map(|b| {
            let pct = ((b as f64 / (10.0 * 1024.0 * 1024.0 * 1024.0)) * 100.0) as u8;
            pct.min(100)
        })
        .unwrap_or(0);

    let db = list_databases();
    let db_count = if db.databases.is_empty() && !db.detail.contains("Listed via") {
        "n/a".into()
    } else {
        db.databases.len().to_string()
    };
    let ftp = detect_ftp();
    let ftp_label: String = if ftp.ready {
        "See FTP hub".to_string()
    } else {
        "0".to_string()
    };

    // Live mode records each view. Minimalist reuses the ring; write only if empty.
    let samples = if minimalist {
        let existing = load_samples();
        if existing.is_empty() {
            record_host_sample()
        } else {
            existing
        }
    } else {
        record_host_sample()
    };
    let cpu_vals: Vec<f32> = samples.iter().map(|s| s.cpu).collect();
    let mem_vals: Vec<f32> = samples.iter().map(|s| s.mem).collect();
    let (cpu_cur, cpu_avg, cpu_peak) = stats_for(&cpu_vals);
    let (mem_cur, mem_avg, mem_peak) = stats_for(&mem_vals);
    let cpu_label = cpu_cur
        .map(|v| format!("{v:.0}%"))
        .unwrap_or_else(|| "n/a".into());
    let mem_label = mem_cur
        .map(|v| format!("{v:.0}%"))
        .unwrap_or_else(|| "n/a".into());
    let bw = bandwidth_for_site(site);
    let cpu_stroke = crate::panel_dashboard::gauge_stroke_for_usage(
        cpu_cur.unwrap_or(0.0).clamp(0.0, 100.0) as u8,
    );
    let mem_stroke = crate::panel_dashboard::gauge_stroke_for_usage(
        mem_cur.unwrap_or(0.0).clamp(0.0, 100.0) as u8,
    );

    let mut cards = String::from(r#"<div class="manage-card-grid">"#);
    cards.push_str(&resource_card("Disk Usage", &disk, Some(disk_pct)));
    cards.push_str(&resource_card_with_hint(
        "Bandwidth",
        &bw.label,
        &bw.hint,
        None,
    ));
    cards.push_str(&resource_card("Databases", &db_count, None));
    cards.push_str(&resource_card("FTP Accounts", &ftp_label, None));
    cards.push_str("</div>");

    let domain_q = html_escape(&site.domain);
    let poll_ms = if minimalist { "0" } else { "10000" };
    let cpu_summary = chart_summary("cpu", minimalist, &cpu_label, cpu_avg, cpu_peak, true);
    let mem_summary = chart_summary("mem", minimalist, &mem_label, mem_avg, mem_peak, false);
    let cpu_svg = metrics_chart_svg(&samples, "cpu", cpu_stroke);
    let mem_svg = metrics_chart_svg(&samples, "mem", mem_stroke);
    let snapshot = metrics_snapshot_script(SnapshotInput {
        site,
        samples: &samples,
        cpu_cur,
        cpu_avg,
        cpu_peak,
        mem_cur,
        mem_avg,
        mem_peak,
        cpu_svg: &cpu_svg,
        mem_svg: &mem_svg,
        bw: &bw,
        minimalist,
    });
    let charts = format!(
        r#"<div class="manage-charts" data-metrics-domain="{domain}" data-metrics-poll="{poll}" data-metrics-minimalist="{mini}">
  <div class="manage-chart" data-metric="cpu">
    <div class="manage-chart-head">
      <h3>CPU Usage (host)</h3>
      <button type="button" class="manage-chart-details" data-metric-details="cpu" aria-haspopup="dialog">Details</button>
    </div>
    <p class="manage-chart-summary">{cpu_summary}</p>
    <div class="manage-chart-svg" data-metric-svg="cpu">{cpu_svg}</div>
    <div class="manage-chart-readout" data-metric-readout="cpu" aria-live="polite">
      <span class="manage-chart-readout-hint">Tap a sample for time and value</span>
      <span class="manage-chart-readout-value" hidden></span>
    </div>
  </div>
  <div class="manage-chart" data-metric="mem">
    <div class="manage-chart-head">
      <h3>Memory Usage (host)</h3>
      <button type="button" class="manage-chart-details" data-metric-details="mem" aria-haspopup="dialog">Details</button>
    </div>
    <p class="manage-chart-summary">{mem_summary}</p>
    <div class="manage-chart-svg" data-metric-svg="mem">{mem_svg}</div>
    <div class="manage-chart-readout" data-metric-readout="mem" aria-live="polite">
      <span class="manage-chart-readout-hint">Tap a sample for time and value</span>
      <span class="manage-chart-readout-value" hidden></span>
    </div>
  </div>
</div>
{snapshot}
<dialog class="manage-metric-dialog" id="manage-metric-dialog">
  <form method="dialog" class="manage-metric-dialog-inner">
    <header>
      <h2 id="manage-metric-dialog-title">Host metrics</h2>
      <button type="submit" class="manage-btn" value="close">Close</button>
    </header>
    <p id="manage-metric-dialog-summary" class="manage-muted"></p>
    <div id="manage-metric-dialog-svg"></div>
    <div class="manage-chart-readout manage-chart-readout-dialog" id="manage-metric-dialog-readout" aria-live="polite">
      <span class="manage-chart-readout-hint">Tap a sample for time and value</span>
      <span class="manage-chart-readout-value" hidden></span>
    </div>
    <ul id="manage-metric-dialog-stats" class="manage-metric-stats"></ul>
  </form>
</dialog>
{script}"#,
        domain = domain_q,
        poll = poll_ms,
        mini = if minimalist { "1" } else { "0" },
        cpu_summary = cpu_summary,
        mem_summary = mem_summary,
        cpu_svg = cpu_svg,
        mem_svg = mem_svg,
        snapshot = snapshot,
        script = overview_metrics_script(),
    );

    let home = site_home_from_record(site);
    let engine = site
        .engine
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or("Not set");
    let stack = detect_web_server_label();
    let legacy = if is_legacy_docroot(&site.docroot) {
        r#"<p class="manage-muted">This site uses a legacy or custom document root. New sites use the domain home under <code>/home/</code>.</p>"#
    } else {
        ""
    };
    let meta = format!(
        r#"<p class="manage-muted">Owner: <strong>{owner}</strong> · Docroot: <code>{docroot}</code> · Home: <code>{home}</code> · Engine: {engine} · Stack: {stack}{internal}</p>{legacy}"#,
        owner = html_escape(&site.owner),
        docroot = html_escape(&site.docroot),
        home = html_escape(&home.display().to_string()),
        engine = html_escape(engine),
        stack = html_escape(&stack),
        internal = site
            .internal_ip
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|ip| format!(" · Internal IP: <code>{}</code>", html_escape(ip)))
            .unwrap_or_default(),
        legacy = legacy,
    );

    format!(
        "{cards}{ssl}{charts}{meta}",
        cards = cards,
        ssl = ssl_status_card(site),
        charts = charts,
        meta = meta,
    )
}

fn chart_summary(
    kind: &str,
    minimalist: bool,
    current: &str,
    avg: Option<f32>,
    peak: Option<f32>,
    not_per_site: bool,
) -> String {
    let avg_s = html_escape(&fmt_pct(avg));
    let peak_s = html_escape(&fmt_pct(peak));
    let cur = html_escape(current);
    let scope = if not_per_site { " Not per-site." } else { "" };
    if minimalist {
        format!(
            "Snapshot (minimalist): <strong data-metric-current=\"{kind}\">{cur}</strong>. Avg <span data-metric-avg=\"{kind}\">{avg}</span>, peak <span data-metric-peak=\"{kind}\">{peak}</span>. Refresh page to update.{scope}",
            kind = kind,
            cur = cur,
            avg = avg_s,
            peak = peak_s,
            scope = scope,
        )
    } else {
        let lead = if kind == "mem" {
            "Live host memory"
        } else {
            "Live host load"
        };
        format!(
            "{lead}: <strong data-metric-current=\"{kind}\">{cur}</strong>. Avg <span data-metric-avg=\"{kind}\">{avg}</span>, peak <span data-metric-peak=\"{kind}\">{peak}</span>.{scope} Samples every ~10s.",
            lead = lead,
            kind = kind,
            cur = cur,
            avg = avg_s,
            peak = peak_s,
            scope = scope,
        )
    }
}

fn pct_json(v: Option<f32>) -> serde_json::Value {
    match v {
        Some(n) => serde_json::json!((n * 10.0).round() / 10.0),
        None => serde_json::Value::Null,
    }
}

struct SnapshotInput<'a> {
    site: &'a SiteRecord,
    samples: &'a [MetricSample],
    cpu_cur: Option<f32>,
    cpu_avg: Option<f32>,
    cpu_peak: Option<f32>,
    mem_cur: Option<f32>,
    mem_avg: Option<f32>,
    mem_peak: Option<f32>,
    cpu_svg: &'a str,
    mem_svg: &'a str,
    bw: &'a BandwidthInfo,
    minimalist: bool,
}

fn metrics_snapshot_script(input: SnapshotInput<'_>) -> String {
    let detail = if input.minimalist {
        "Host metrics snapshot (minimalist). Refresh the page to update."
    } else {
        "Host live metrics (not per-site). Site-level CPU/bandwidth metering ships later."
    };
    let payload = serde_json::json!({
        "ok": true,
        "scope": "host",
        "domain": input.site.domain,
        "window_seconds": WINDOW_SECS,
        "minimalist": input.minimalist,
        "detail": detail,
        "cpu": {
            "current": pct_json(input.cpu_cur),
            "avg": pct_json(input.cpu_avg),
            "peak": pct_json(input.cpu_peak),
            "svg": input.cpu_svg,
        },
        "mem": {
            "current": pct_json(input.mem_cur),
            "avg": pct_json(input.mem_avg),
            "peak": pct_json(input.mem_peak),
            "svg": input.mem_svg,
        },
        "bandwidth": {
            "label": input.bw.label,
            "hint": input.bw.hint,
            "bytes": input.bw.bytes,
            "bytes_label": input.bw.bytes.map(format_bytes),
            "period": input.bw.period,
            "source": input.bw.source,
            "quota_mb": input.bw.quota_mb,
        },
        "samples": input.samples.iter().map(|s| serde_json::json!({
            "t": s.t,
            "cpu": s.cpu,
            "mem": s.mem,
        })).collect::<Vec<_>>(),
    });
    let raw = serde_json::to_string(&payload).unwrap_or_else(|_| "{\"ok\":false}".into());
    let safe = raw.replace('<', "\\u003c");
    format!(r#"<script type="application/json" id="manage-metrics-snapshot">{safe}</script>"#)
}

fn fmt_pct(v: Option<f32>) -> String {
    v.map(|n| format!("{n:.0}%"))
        .unwrap_or_else(|| "n/a".into())
}

fn resource_card_with_hint(label: &str, value: &str, hint: &str, pct: Option<u8>) -> String {
    let bar = match pct {
        Some(p) => format!(
            r#"<div class="bar" aria-hidden="true"><i style="width:{}%"></i></div>"#,
            p.min(100)
        ),
        None => String::new(),
    };
    format!(
        r#"<div class="manage-stat" title="{hint}"><span>{label}</span><strong>{value}</strong><em class="manage-stat-hint">{hint}</em>{bar}</div>"#,
        label = html_escape(label),
        value = html_escape(value),
        hint = html_escape(hint),
        bar = bar,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_user_prefs::save_user_minimalist_mode;

    fn site() -> SiteRecord {
        SiteRecord {
            schema_version: 1,
            domain: "cpn-lab-test.example".into(),
            owner: "Admin".into(),
            docroot: "/tmp/cpn-manage-missing".into(),
            enabled: true,
            engine: None,
            notes: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
            vhost_wired: false,
            ssl: Default::default(),
            internal_ip: None,
            owner_suspend_message: String::new(),
            suspended_by: None,
            php_version: None,
        }
    }

    #[test]
    fn overview_has_resource_cards() {
        with_test_data_dir(|| {
            let html = tab_overview(&site(), "Admin");
            assert!(html.contains("Disk Usage"));
            assert!(html.contains("Bandwidth"));
            assert!(html.contains("CPU Usage (host)"));
            assert!(html.contains("Memory Usage (host)"));
            assert!(html.contains("/api/websites/manage/metrics"));
            assert!(html.contains("manage-metric-dialog"));
            assert!(html.contains("manage-chart-readout"));
            assert!(html.contains("Samples every ~10s"));
            assert!(html.contains("data-metric-details"));
            assert!(html.contains("data-metrics-poll=\"10000\""));
            assert!(html.contains("Live host load:"));
            assert!(html.contains("data-metrics-minimalist=\"0\""));
            assert!(!html.to_lowercase().contains("email marketing"));
            assert!(!html.to_lowercase().contains("cyberpanel"));
            assert!(!html.contains('\u{2014}'));
            assert!(!html.contains('\u{2013}'));
            assert!(!html.contains("SSL material"));
            assert!(html.contains("manage-ssl") || html.contains("No SSL"));
        });
    }

    #[test]
    fn overview_minimalist_disables_poll() {
        with_test_data_dir(|| {
            save_user_minimalist_mode("Admin", true).unwrap();
            let html = tab_overview(&site(), "Admin");
            assert!(html.contains("data-metrics-poll=\"0\""));
            assert!(html.contains("data-metrics-minimalist=\"1\""));
            assert!(html.contains("Snapshot (minimalist)"));
            assert!(html.contains("Refresh page to update"));
            assert!(html.contains("manage-metrics-snapshot"));
            assert!(html.contains("manage-chart-readout"));
            assert!(!html.contains("Live host load:"));
            assert!(!html.contains("Live host memory:"));
            assert!(!html.contains('\u{2014}'));
            assert!(!html.contains('\u{2013}'));
        });
    }
}

//! Manage Overview tab: disk/bandwidth cards plus live host CPU/memory charts.

use crate::panel_ops_db::list_databases;
use crate::panel_ops_ftp::detect_ftp;
use crate::panel_website_bandwidth::bandwidth_for_site;
use crate::panel_website_manage_ui::{html_escape, resource_card, ssl_status_card};
use crate::panel_website_metrics_chart::metrics_chart_svg;
use crate::panel_website_metrics_ring::{record_host_sample, stats_for};
use crate::panel_website_resources::{approx_dir_bytes, format_bytes};
use crate::service_detect::detect_web_server_label;
use crate::sites::{SiteRecord, is_legacy_docroot, site_home_from_record};
use std::path::Path;

pub fn tab_overview(site: &SiteRecord) -> String {
    let disk_bytes = approx_dir_bytes(Path::new(&site.docroot), 8_000);
    let disk = disk_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "Unavailable".into());
    // Soft quota hint: 10 GB visual only until Packages merge.
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

    let samples = record_host_sample();
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
    let charts = format!(
        r#"<div class="manage-charts" data-metrics-domain="{domain}" data-metrics-poll="10000">
  <button type="button" class="manage-chart manage-chart-btn" data-metric="cpu" aria-haspopup="dialog">
    <h3>CPU Usage (host)</h3>
    <p class="manage-chart-summary">Live host load: <strong data-metric-current="cpu">{cpu}</strong>. Avg <span data-metric-avg="cpu">{cpu_avg}</span>, peak <span data-metric-peak="cpu">{cpu_peak}</span>. Not per-site. Tap for details.</p>
    <div class="manage-chart-svg" data-metric-svg="cpu">{cpu_svg}</div>
  </button>
  <button type="button" class="manage-chart manage-chart-btn" data-metric="mem" aria-haspopup="dialog">
    <h3>Memory Usage (host)</h3>
    <p class="manage-chart-summary">Live host memory: <strong data-metric-current="mem">{mem}</strong>. Avg <span data-metric-avg="mem">{mem_avg}</span>, peak <span data-metric-peak="mem">{mem_peak}</span>. Tap for details.</p>
    <div class="manage-chart-svg" data-metric-svg="mem">{mem_svg}</div>
  </button>
</div>
<dialog class="manage-metric-dialog" id="manage-metric-dialog">
  <form method="dialog" class="manage-metric-dialog-inner">
    <header>
      <h2 id="manage-metric-dialog-title">Host metrics</h2>
      <button type="submit" class="manage-btn" value="close">Close</button>
    </header>
    <p id="manage-metric-dialog-summary" class="manage-muted"></p>
    <div id="manage-metric-dialog-svg"></div>
    <ul id="manage-metric-dialog-stats" class="manage-metric-stats"></ul>
  </form>
</dialog>
{script}"#,
        domain = domain_q,
        cpu = html_escape(&cpu_label),
        mem = html_escape(&mem_label),
        cpu_avg = html_escape(&fmt_pct(cpu_avg)),
        cpu_peak = html_escape(&fmt_pct(cpu_peak)),
        mem_avg = html_escape(&fmt_pct(mem_avg)),
        mem_peak = html_escape(&fmt_pct(mem_peak)),
        cpu_svg = metrics_chart_svg(&samples, "cpu", cpu_stroke),
        mem_svg = metrics_chart_svg(&samples, "mem", mem_stroke),
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

fn overview_metrics_script() -> &'static str {
    r#"<script>
(function () {
  var root = document.querySelector(".manage-charts[data-metrics-domain]");
  if (!root) return;
  var domain = root.getAttribute("data-metrics-domain") || "";
  var pollMs = parseInt(root.getAttribute("data-metrics-poll") || "10000", 10);
  var dialog = document.getElementById("manage-metric-dialog");
  var titleEl = document.getElementById("manage-metric-dialog-title");
  var summaryEl = document.getElementById("manage-metric-dialog-summary");
  var svgEl = document.getElementById("manage-metric-dialog-svg");
  var statsEl = document.getElementById("manage-metric-dialog-stats");
  var lastPayload = null;

  function pct(v) {
    return (v === null || v === undefined) ? "n/a" : (Math.round(v) + "%");
  }

  function applyPayload(data) {
    lastPayload = data;
    if (!data || !data.ok) return;
    ["cpu", "mem"].forEach(function (kind) {
      var block = data[kind];
      if (!block) return;
      var cur = root.querySelector('[data-metric-current="' + kind + '"]');
      var avg = root.querySelector('[data-metric-avg="' + kind + '"]');
      var peak = root.querySelector('[data-metric-peak="' + kind + '"]');
      var svgWrap = root.querySelector('[data-metric-svg="' + kind + '"]');
      if (cur) cur.textContent = pct(block.current);
      if (avg) avg.textContent = pct(block.avg);
      if (peak) peak.textContent = pct(block.peak);
      if (svgWrap && block.svg) svgWrap.innerHTML = block.svg;
    });
    var bwHint = document.querySelector(".manage-stat-hint");
    if (data.bandwidth && bwHint) {
      var card = bwHint.closest(".manage-stat");
      if (card) {
        var strong = card.querySelector("strong");
        if (strong && data.bandwidth.label) strong.textContent = data.bandwidth.label;
      }
      if (data.bandwidth.hint) {
        bwHint.textContent = data.bandwidth.hint;
        if (card) card.setAttribute("title", data.bandwidth.hint);
      }
    }
  }

  function openMetric(kind) {
    if (!dialog || !lastPayload || !lastPayload[kind]) return;
    var block = lastPayload[kind];
    var name = kind === "mem" ? "Memory Usage (host)" : "CPU Usage (host)";
    if (titleEl) titleEl.textContent = name;
    if (summaryEl) {
      summaryEl.textContent = (lastPayload.detail || "Host live metrics.") +
        " Window: last " + Math.round((lastPayload.window_seconds || 900) / 60) + " minutes.";
    }
    if (svgEl) svgEl.innerHTML = block.svg || "";
    if (statsEl) {
      statsEl.innerHTML =
        "<li>Current: <strong>" + pct(block.current) + "</strong></li>" +
        "<li>Average: <strong>" + pct(block.avg) + "</strong></li>" +
        "<li>Peak: <strong>" + pct(block.peak) + "</strong></li>" +
        "<li>Samples: <strong>" + ((lastPayload.samples && lastPayload.samples.length) || 0) + "</strong></li>";
    }
    if (typeof dialog.showModal === "function") dialog.showModal();
  }

  function poll() {
    fetch("/api/websites/manage/metrics?domain=" + encodeURIComponent(domain), {
      credentials: "same-origin",
      headers: { "Accept": "application/json" }
    }).then(function (r) { return r.json(); }).then(applyPayload).catch(function () {});
  }

  root.querySelectorAll(".manage-chart-btn").forEach(function (btn) {
    btn.addEventListener("click", function () {
      openMetric(btn.getAttribute("data-metric") || "cpu");
    });
  });

  poll();
  if (pollMs > 0) setInterval(poll, pollMs);
})();
</script>"#
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let html = tab_overview(&site());
        assert!(html.contains("Disk Usage"));
        assert!(html.contains("Bandwidth"));
        assert!(html.contains("CPU Usage (host)"));
        assert!(html.contains("Memory Usage (host)"));
        assert!(html.contains("/api/websites/manage/metrics"));
        assert!(html.contains("manage-metric-dialog"));
        assert!(!html.to_lowercase().contains("email marketing"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }
}

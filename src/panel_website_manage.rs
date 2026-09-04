//! Website Manage page HTML for CPN Panel.

use crate::service_detect::detect_web_server_label;
use crate::sites::{
    SiteRecord, is_legacy_docroot, list_sites, resolve_parent_domain, site_home_from_record,
};
use crate::website_preview::{PreviewSize, preview_card_html, public_site_url};
use std::path::Path;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn section_heading(title: &str, blurb: &str) -> String {
    format!(
        r#"
      <div class="dashboard-heading">
        <div>
          <p class="eyebrow">CPN PANEL</p>
          <h1>{title}</h1>
          <p>{blurb}</p>
        </div>
      </div>"#,
        title = html_escape(title),
        blurb = html_escape(blurb),
    )
}

fn notice_block(kind: &str, message: Option<&str>) -> String {
    let Some(message) = message.filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let class = if kind == "error" {
        "panel-notice error"
    } else {
        "panel-notice ok"
    };
    format!(
        r#"<p class="{class}" role="status">{msg}</p>"#,
        msg = html_escape(message)
    )
}

/// Cheap approximate disk usage (bytes) with a file walk cap.
pub fn approx_dir_bytes(root: &Path, max_files: usize) -> Option<u64> {
    if !root.exists() {
        return None;
    }
    let mut total = 0u64;
    let mut count = 0usize;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
            count += 1;
            if count >= max_files {
                return Some(total);
            }
        }
    }
    Some(total)
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

fn child_sites(parent: &str) -> Vec<SiteRecord> {
    let Ok(all) = list_sites() else {
        return Vec::new();
    };
    all.into_iter()
        .filter(|site| {
            resolve_parent_domain(&site.domain)
                .ok()
                .flatten()
                .as_deref()
                == Some(parent)
        })
        .collect()
}

pub fn website_manage_main(site: &SiteRecord, notice: Option<&str>, error: Option<&str>) -> String {
    let status = if site.enabled { "Active" } else { "Suspended" };
    let wired = if site.vhost_wired {
        "Vhost wired"
    } else {
        "Files ready (vhost later)"
    };
    let home = site_home_from_record(site);
    let disk = approx_dir_bytes(Path::new(&site.docroot), 8_000)
        .map(format_bytes)
        .unwrap_or_else(|| "Unavailable".into());
    let engine = site
        .engine
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or("Not set");
    let stack = detect_web_server_label();
    let legacy = if is_legacy_docroot(&site.docroot) {
        r#"<p class="muted">This site uses a legacy or custom document root. New sites use the domain home under <code>/home/</code>.</p>"#
    } else {
        ""
    };
    let parent = resolve_parent_domain(&site.domain)
        .ok()
        .flatten()
        .unwrap_or_default();
    let parent_line = if parent.is_empty() {
        String::new()
    } else {
        format!(
            r#"<li><span>Parent</span><strong><a href="/websites/manage?domain={p}">{p}</a></strong></li>"#,
            p = html_escape(&parent),
        )
    };
    let children = child_sites(&site.domain);
    let mut sub_block = String::new();
    if parent.is_empty() {
        if children.is_empty() {
            sub_block.push_str(
                r#"<p class="muted">No subdomains registered under this domain yet.</p>"#,
            );
        } else {
            sub_block.push_str(r#"<ul class="kv-list">"#);
            for child in &children {
                let st = if child.enabled { "Active" } else { "Suspended" };
                sub_block.push_str(&format!(
                    r#"<li><span>{domain}</span><strong>{st}, <a href="/websites/manage?domain={domain}">Manage</a></strong></li>"#,
                    domain = html_escape(&child.domain),
                    st = st,
                ));
            }
            sub_block.push_str("</ul>");
        }
    }
    let suspend_form = if site.enabled {
        format!(
            r#"<form method="post" action="/websites/suspend" class="inline-form" onsubmit="return confirm('Suspend {domain}?');">
          <input type="hidden" name="domain" value="{domain}">
          <button type="submit" class="btn-warn" style="min-height:40px;padding:0 14px;border:0;border-radius:999px;background:#fffaeb;color:#b54708;font-weight:700;">Suspend</button>
        </form>"#,
            domain = html_escape(&site.domain),
        )
    } else {
        format!(
            r#"<form method="post" action="/websites/resume" class="inline-form">
          <input type="hidden" name="domain" value="{domain}">
          <button type="submit" class="btn-primary">Resume</button>
        </form>"#,
            domain = html_escape(&site.domain),
        )
    };
    let domain_q = html_escape(&site.domain);
    let preview = preview_card_html(&site.domain, PreviewSize::Manage).unwrap_or_else(|_| {
        let fallback_url = public_site_url(&site.domain)
            .unwrap_or_else(|_| format!("http://{}", site.domain));
        format!(
            r#"<div class="site-preview site-preview--manage">
  <div class="site-preview-viewport">
    <div class="site-preview-fallback" style="display:flex;">
      <div class="site-preview-fallback-inner">
        <strong>{domain}</strong>
        <p>Preview unavailable.</p>
      </div>
    </div>
  </div>
  <div class="site-preview-actions">
    <a class="site-preview-visit" href="{url}" target="_blank" rel="noopener noreferrer">Visit Site</a>
  </div>
</div>"#,
            domain = html_escape(&site.domain),
            url = html_escape(&fallback_url),
        )
    });
    format!(
        r##"{heading}
      {ok}
      {err}
      <article class="section-card">
        <p class="muted"><a href="/websites">Back to Websites</a></p>
        <h2>{domain}</h2>
        <div class="plugin-actions" style="display:flex;flex-wrap:wrap;gap:8px;margin:14px 0 18px;">
          <a class="btn-primary" href="#overview">Overview</a>
          <a class="btn-secondary" href="#settings" style="background:#f2f4f7;color:#344054;border-radius:999px;padding:0 14px;min-height:40px;display:inline-flex;align-items:center;font-weight:700;">Settings</a>
          <a class="btn-secondary" href="#docroot" style="background:#f2f4f7;color:#344054;border-radius:999px;padding:0 14px;min-height:40px;display:inline-flex;align-items:center;font-weight:700;">File manager</a>
          {suspend}
          <form method="post" action="/websites/delete" class="inline-form" onsubmit="return confirm('Delete site {domain}? Document files under /home are kept.');">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="btn-danger">Delete</button>
          </form>
        </div>
        <h3 id="overview">Overview</h3>
        <div class="site-manage-layout">
          <aside class="site-preview-col" aria-label="Website preview">
            {preview}
          </aside>
          <div class="site-manage-details">
            <ul class="kv-list" style="margin-top:0;">
              <li><span>Status</span><strong>{status}</strong></li>
              <li><span>Owner</span><strong>{owner}</strong></li>
              <li><span>Provisioning</span><strong>{wired}</strong></li>
              <li><span>Document root</span><strong><code>{docroot}</code></strong></li>
              <li><span>Site home</span><strong><code>{home}</code></strong></li>
              <li><span>Disk (approx.)</span><strong>{disk}</strong></li>
              <li><span>Engine hint</span><strong>{engine}</strong></li>
              <li><span>Detected web stack</span><strong>{stack}</strong></li>
              {parent_line}
            </ul>
            {legacy}
          </div>
        </div>
        <h3 id="docroot" style="margin-top:22px;">Document root</h3>
        <p>Files for this site are in <code>{docroot}</code>.</p>
        <p class="muted">A full in-panel file manager ships later. Until then, manage files on the host at that path.</p>
        <h3 id="settings" style="margin-top:22px;">Quick links</h3>
        <p class="plugin-actions" style="display:flex;flex-wrap:wrap;gap:8px;">
          <a class="btn-secondary" style="background:#f2f4f7;color:#344054;border-radius:999px;padding:0 14px;min-height:40px;display:inline-flex;align-items:center;font-weight:700;" href="/email">Email</a>
          <a class="btn-secondary" style="background:#f2f4f7;color:#344054;border-radius:999px;padding:0 14px;min-height:40px;display:inline-flex;align-items:center;font-weight:700;" href="/plugins?domain={domain_q}">Plugins</a>
          <a class="btn-secondary" style="background:#f2f4f7;color:#344054;border-radius:999px;padding:0 14px;min-height:40px;display:inline-flex;align-items:center;font-weight:700;" href="/backups?scope=site&amp;domain={domain_q}">Backups</a>
          <a class="btn-secondary" style="background:#f2f4f7;color:#344054;border-radius:999px;padding:0 14px;min-height:40px;display:inline-flex;align-items:center;font-weight:700;" href="/apps?domain={domain_q}">Apps</a>
        </p>
        <h3 style="margin-top:22px;">Subdomains</h3>
        {subs}
      </article>"##,
        heading = section_heading(&site.domain, "Manage this website and related tools."),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        domain = html_escape(&site.domain),
        domain_q = domain_q,
        preview = preview,
        status = status,
        owner = html_escape(&site.owner),
        wired = wired,
        docroot = html_escape(&site.docroot),
        home = html_escape(&home.display().to_string()),
        disk = html_escape(&disk),
        engine = html_escape(engine),
        stack = html_escape(&stack),
        parent_line = parent_line,
        legacy = legacy,
        suspend = suspend_form,
        subs = sub_block,
    )
}

#[cfg(test)]
mod tests {
    use super::website_manage_main;
    use crate::sites::SiteRecord;

    #[test]
    fn manage_page_includes_preview_column() {
        let site = SiteRecord {
            schema_version: 1,
            domain: "cpn-lab-test.example".into(),
            owner: "Admin".into(),
            docroot: "/tmp/cpn-preview-missing".into(),
            enabled: true,
            engine: None,
            notes: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
            vhost_wired: false,
        };
        let html = website_manage_main(&site, None, None);
        assert!(html.contains("site-manage-layout"));
        assert!(html.contains("site-preview-col"));
        assert!(html.contains("Visit Site"));
        assert!(html.contains("data-site-preview"));
        assert!(html.contains("http://cpn-lab-test.example"));
        assert!(html.contains("rel=\"noopener noreferrer\""));
    }
}

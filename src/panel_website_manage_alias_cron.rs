//! Manage UI for Domain Alias and Cron Jobs tabs.

use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_site_alias::alias_csrf_token;
use crate::panel_ops_site_cron::{SiteCronJob, cron_csrf_token, list_site_cron_jobs};
use crate::panel_website_manage_ui::{html_escape, section};
use crate::sites::SiteRecord;

pub fn tab_alias(site: &SiteRecord, username: &str) -> String {
    let domain_q = html_escape(&site.domain);
    let csrf = html_escape(&alias_csrf_token(username));
    let cf_note = if cloudflare_configured() {
        r#"<label class="manage-muted" style="display:block;margin:8px 0;">DNS (optional)
<select name="dns_mode">
  <option value="cname">Create Cloudflare CNAME to primary</option>
  <option value="a">Create Cloudflare A record (server IP)</option>
  <option value="none">Skip Cloudflare DNS</option>
</select></label>"#
    } else {
        r#"<p class="manage-muted">Cloudflare API is not configured on this host (<code>/var/lib/cpn/cloudflare.json</code>). Alias is still stored and applied to the web stack.</p>"#
    };

    let mut rows = String::from(
        r#"<table class="manage-table" style="width:100%;border-collapse:collapse;margin-top:12px;"><thead><tr><th align="left">Hostname</th><th></th></tr></thead><tbody>"#,
    );
    if site.aliases.is_empty() {
        rows.push_str(
            r#"<tr><td colspan="2" class="manage-muted">No aliases yet. Add a hostname that should serve this site.</td></tr>"#,
        );
    } else {
        for alias in &site.aliases {
            let a = html_escape(alias);
            rows.push_str(&format!(
                r#"<tr><td><code>{a}</code></td><td>
<form method="post" action="/websites/alias/remove" style="display:inline;" onsubmit="return confirm('Remove alias {a}?');">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="domain" value="{domain_q}">
  <input type="hidden" name="alias" value="{a}">
  <button type="submit" class="btn-danger">Remove</button>
</form></td></tr>"#
            ));
        }
    }
    rows.push_str("</tbody></table>");

    let form = format!(
        r#"<form method="post" action="/websites/alias/add" class="manage-form" style="margin-top:12px;max-width:520px;">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="domain" value="{domain_q}">
  <label>Hostname alias
    <input type="text" name="alias" required maxlength="253" placeholder="www.{domain_q}" pattern="[A-Za-z0-9.-]+" style="width:100%;">
  </label>
  {cf}
  <button type="submit" class="btn-primary" style="margin-top:10px;">Add alias</button>
</form>"#,
        csrf = csrf,
        domain_q = domain_q,
        cf = cf_note,
    );

    let help = r#"<p class="manage-muted">Aliases are stored in the site registry, written under <code>/var/lib/cpn/vhosts/</code>, and applied as OLS map / Apache ServerAlias / nginx <code>server_name</code> when that stack is present.</p>"#;

    format!(
        "{intro}{form}{list}",
        intro = section("Domain Alias", &format!("{help}{rows}")),
        form = section("Add alias", &form),
        list = "",
    )
}

pub fn tab_cron(site: &SiteRecord, username: &str) -> String {
    let domain_q = html_escape(&site.domain);
    let csrf = html_escape(&cron_csrf_token(username));
    let jobs = list_site_cron_jobs(&site.domain)
        .map(|(_, jobs)| jobs)
        .unwrap_or_default();
    let home = crate::sites::site_home_from_record(site);
    let home_esc = html_escape(&home.display().to_string());

    let mut rows = String::from(
        r#"<table class="manage-table" style="width:100%;border-collapse:collapse;"><thead><tr>
<th align="left">Schedule</th><th align="left">Command</th><th>On</th><th></th></tr></thead><tbody>"#,
    );
    if jobs.is_empty() {
        rows.push_str(
            r#"<tr><td colspan="4" class="manage-muted">No cron jobs yet for this site.</td></tr>"#,
        );
    } else {
        for job in &jobs {
            rows.push_str(&cron_row(site, &csrf, &domain_q, job));
        }
    }
    rows.push_str("</tbody></table>");

    let add = format!(
        r#"<form method="post" action="/websites/cron/add" class="manage-form" style="margin-top:12px;max-width:640px;">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="domain" value="{domain_q}">
  <p class="manage-muted">Runs with cwd jailed to <code>{home}</code>. Prefer <code>php public_html/…</code> (no shell metacharacters).</p>
  <div style="display:flex;flex-wrap:wrap;gap:8px;">
    <label>Minute <input name="minute" value="*/5" required style="width:72px;"></label>
    <label>Hour <input name="hour" value="*" required style="width:72px;"></label>
    <label>Day <input name="day" value="*" required style="width:72px;"></label>
    <label>Month <input name="month" value="*" required style="width:72px;"></label>
    <label>Weekday <input name="weekday" value="*" required style="width:72px;"></label>
  </div>
  <label style="display:block;margin-top:8px;">Command
    <input type="text" name="command" required maxlength="400" placeholder="php public_html/cron.php" style="width:100%;">
  </label>
  <label style="display:block;margin-top:8px;">Comment
    <input type="text" name="comment" maxlength="120" style="width:100%;">
  </label>
  <button type="submit" class="btn-primary" style="margin-top:10px;">Add cron job</button>
</form>"#,
        csrf = csrf,
        domain_q = domain_q,
        home = home_esc,
    );

    format!(
        "{list}{add}",
        list = section("Cron Jobs", &rows),
        add = section("Add cron job", &add),
    )
}

fn cron_row(site: &SiteRecord, csrf: &str, domain_q: &str, job: &SiteCronJob) -> String {
    let _ = site;
    let id = html_escape(&job.id);
    let sched = html_escape(&format!(
        "{} {} {} {} {}",
        job.minute, job.hour, job.day, job.month, job.weekday
    ));
    let cmd = html_escape(&job.command);
    let on = if job.enabled { "yes" } else { "no" };
    let enabled_checked = if job.enabled { " checked" } else { "" };
    format!(
        r#"<tr>
<td><code>{sched}</code></td>
<td><code>{cmd}</code></td>
<td>{on}</td>
<td style="white-space:nowrap;">
<form method="post" action="/websites/cron/update" style="display:inline;">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="domain" value="{domain_q}">
  <input type="hidden" name="job_id" value="{id}">
  <input type="hidden" name="minute" value="{m}">
  <input type="hidden" name="hour" value="{h}">
  <input type="hidden" name="day" value="{d}">
  <input type="hidden" name="month" value="{mo}">
  <input type="hidden" name="weekday" value="{w}">
  <input type="hidden" name="command" value="{cmd}">
  <input type="hidden" name="comment" value="{c}">
  <label><input type="checkbox" name="enabled" value="1"{enabled_checked}> On</label>
  <button type="submit" class="btn-secondary">Save</button>
</form>
<form method="post" action="/websites/cron/delete" style="display:inline;" onsubmit="return confirm('Delete this cron job?');">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="domain" value="{domain_q}">
  <input type="hidden" name="job_id" value="{id}">
  <button type="submit" class="btn-danger">Delete</button>
</form>
</td></tr>"#,
        sched = sched,
        cmd = cmd,
        on = on,
        csrf = csrf,
        domain_q = domain_q,
        id = id,
        m = html_escape(&job.minute),
        h = html_escape(&job.hour),
        d = html_escape(&job.day),
        mo = html_escape(&job.month),
        w = html_escape(&job.weekday),
        c = html_escape(&job.comment),
        enabled_checked = enabled_checked,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> SiteRecord {
        SiteRecord {
            schema_version: 4,
            domain: "cpn-lab-test.example".into(),
            owner: "Admin".into(),
            docroot: "/tmp/cpn-manage-missing/public_html".into(),
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
            aliases: vec!["www.cpn-lab-test.example".into()],
        }
    }

    #[test]
    fn alias_tab_lists_and_form() {
        let html = tab_alias(&site(), "Admin");
        assert!(html.contains("Domain Alias"));
        assert!(html.contains("www.cpn-lab-test.example"));
        assert!(html.contains("/websites/alias/add"));
        assert!(!html.to_lowercase().contains("scaffold"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn cron_tab_has_editor() {
        let html = tab_cron(&site(), "Admin");
        assert!(html.contains("Cron Jobs"));
        assert!(html.contains("/websites/cron/add"));
        assert!(!html.to_lowercase().contains("scaffold"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }
}

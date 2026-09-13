//! File Manager UI (classic hosting file manager layout, CPN branding).

use crate::panel_brand::brand_mark_svg;
use crate::panel_hub_http::urlencoding_simple;
use crate::panel_hub_pages_files_assets::{fm_script, fm_styles};
use crate::panel_hubs::{feature_shell, notice_block};
use crate::panel_ops_files::files_csrf_token;
use crate::panel_ops_path::{list_dir, resolve_under_jail};
use std::path::Path;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// View configuration for Root or Site File Manager.
pub struct FilesPageOpts<'a> {
    pub username: &'a str,
    pub path_q: &'a str,
    pub jail_root: &'a Path,
    pub base_url: &'a str,
    pub op_url: &'a str,
    pub upload_url: &'a str,
    /// Extra query prefix ending with `&` when non-empty (e.g. `domain=x&`).
    pub query_extra: &'a str,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub brand_sub: &'a str,
    pub risk_note: &'a str,
    pub crumbs: &'a [(&'a str, Option<&'a str>)],
    pub notice: Option<&'a str>,
    pub error: Option<&'a str>,
    pub edit_path: Option<&'a str>,
    pub edit_content: Option<&'a str>,
    /// Hidden domain field for site FM forms.
    pub domain: Option<&'a str>,
}

fn page_href(opts: &FilesPageOpts<'_>, path: &str) -> String {
    format!(
        "{}?{}path={}",
        opts.base_url,
        opts.query_extra,
        urlencoding_simple(path)
    )
}

fn domain_hidden(domain: Option<&str>) -> String {
    match domain {
        Some(d) if !d.is_empty() => format!(
            r#"<input type="hidden" name="domain" value="{}">"#,
            html_escape(d)
        ),
        _ => String::new(),
    }
}

pub fn files_page(opts: &FilesPageOpts<'_>) -> String {
    let csrf = files_csrf_token(opts.username);
    let home = opts.jail_root.display().to_string();
    let resolved = resolve_under_jail(opts.path_q, opts.jail_root);
    let body = match resolved {
        Ok(path) => match list_dir(&path) {
            Ok(entries) => {
                let path_s = path.display().to_string();
                let parent_href = path
                    .parent()
                    .filter(|p| {
                        resolve_under_jail(&p.display().to_string(), opts.jail_root).is_ok()
                    })
                    .map(|p| page_href(opts, &p.display().to_string()))
                    .unwrap_or_else(|| page_href(opts, &home));
                let mut rows = String::new();
                for ent in &entries {
                    let child = path.join(&ent.basename);
                    let child_s = child.display().to_string();
                    let name_cell = if ent.is_dir {
                        format!(
                            r#"<a href="{href}">{label}</a>"#,
                            href = page_href(opts, &child_s),
                            label = html_escape(&ent.label),
                        )
                    } else {
                        html_escape(&ent.label)
                    };
                    let size_kb = if ent.is_dir {
                        "-".into()
                    } else {
                        format!("{:.1}", ent.size as f64 / 1024.0)
                    };
                    rows.push_str(&format!(
                        r#"<tr>
                      <td><input type="checkbox" class="fm-check" value="{base}"></td>
                      <td class="fm-name">{name}</td>
                      <td>{size}</td>
                      <td>{mtime}</td>
                      <td><code>{mode}</code></td>
                    </tr>"#,
                        base = html_escape(&ent.basename),
                        name = name_cell,
                        size = size_kb,
                        mtime = html_escape(&ent.mtime_label),
                        mode = html_escape(&ent.mode_label),
                    ));
                }
                let tree = build_tree_html(opts, &home, &path_s);
                let editor = edit_modal(opts, &csrf, &path_s);
                let notices = format!(
                    "{}{}",
                    notice_block("ok", opts.notice),
                    notice_block("error", opts.error)
                );
                let risk = format!(r#"<p class="muted">{}</p>"#, html_escape(opts.risk_note));
                format!(
                    r#"{styles}
{risk}
{notices}
<div class="fm-root" id="fm-root" data-path="{path_esc}" data-csrf="{csrf}" data-base="{base}" data-op="{op}" data-upload="{upload}" data-qs="{qs}">
  <div class="fm-brandbar">
    <span class="fm-logo">{logo}</span>
    <strong>CPN Panel</strong>
    <span class="fm-brand-sub">{brand}</span>
  </div>
  <div class="fm-toolbar" role="toolbar" aria-label="File actions">
    <label class="fm-tool"><input type="file" id="fm-upload" hidden multiple>Upload</label>
    <button type="button" class="fm-tool" data-op="create">New File</button>
    <button type="button" class="fm-tool" data-op="mkdir">New Folder</button>
    <button type="button" class="fm-tool" data-op="delete">Delete</button>
    <button type="button" class="fm-tool" data-op="copy">Copy</button>
    <button type="button" class="fm-tool" data-op="move">Move</button>
    <button type="button" class="fm-tool" data-op="rename">Rename</button>
    <button type="button" class="fm-tool" data-op="edit">Edit</button>
    <button type="button" class="fm-tool" data-op="compress">Compress</button>
    <button type="button" class="fm-tool" data-op="extract">Extract</button>
  </div>
  <div class="fm-navrow">
    <a class="fm-navbtn" href="{home_href}">Home</a>
    <a class="fm-navbtn" href="{back}">Back</a>
    <a class="fm-navbtn" href="{refresh}">Refresh</a>
    <button type="button" class="fm-navbtn" id="fm-select-all">Select All</button>
    <button type="button" class="fm-navbtn" id="fm-unselect-all">UnSelect All</button>
    <button type="button" class="fm-navbtn fm-tree-toggle" id="fm-tree-toggle" aria-expanded="true">Tree</button>
  </div>
  <div class="fm-body">
    <aside class="fm-tree" id="fm-tree">
      <div class="fm-tree-title">Current Path</div>
      <code class="fm-cwd">{path_esc}</code>
      {tree}
    </aside>
    <div class="fm-main">
      <form method="get" action="{base}" class="fm-pathform">
        {domain_get}
        <label for="fm-path">Path</label>
        <input id="fm-path" name="path" type="text" value="{path_esc}">
        <button type="submit" class="btn-primary">Open</button>
      </form>
      <form id="fm-op" method="post" action="{op}" class="fm-hidden-form">
        <input type="hidden" name="csrf" value="{csrf}">
        {domain_post}
        <input type="hidden" name="path" value="{path_esc}">
        <input type="hidden" name="op" id="fm-op-field" value="">
        <input type="hidden" name="dest" id="fm-dest-field" value="">
        <input type="hidden" name="new_name" id="fm-new-name" value="">
        <input type="hidden" name="archive_name" id="fm-archive-name" value="">
        <input type="hidden" name="names_csv" id="fm-names-csv" value="">
      </form>
      <div class="table-wrap fm-table-wrap">
        <table class="data-table fm-table">
          <thead><tr><th></th><th>File Name</th><th>Size (KB)</th><th>Last Modified</th><th>Permissions</th></tr></thead>
          <tbody>{rows}</tbody>
        </table>
      </div>
    </div>
  </div>
</div>
{editor}
{script}"#,
                    styles = fm_styles(),
                    risk = risk,
                    notices = notices,
                    path_esc = html_escape(&path_s),
                    csrf = html_escape(&csrf),
                    logo = brand_mark_svg(),
                    brand = html_escape(opts.brand_sub),
                    base = html_escape(opts.base_url),
                    op = html_escape(opts.op_url),
                    upload = html_escape(opts.upload_url),
                    qs = html_escape(opts.query_extra),
                    home_href = page_href(opts, &home),
                    back = parent_href,
                    refresh = page_href(opts, &path_s),
                    domain_get = domain_hidden(opts.domain),
                    domain_post = domain_hidden(opts.domain),
                    tree = tree,
                    rows = rows,
                    editor = editor,
                    script = fm_script(),
                )
            }
            Err(err) => format!(
                "{}{}",
                notice_block("error", Some(&err)),
                notice_block("ok", opts.notice)
            ),
        },
        Err(err) => format!(
            "{}{}",
            notice_block("error", Some(&err)),
            notice_block("ok", opts.notice)
        ),
    };
    feature_shell(opts.crumbs, opts.title, opts.subtitle, &body, None, None)
}

/// Convenience wrapper for admin Root File Manager.
pub fn root_files_page(
    username: &str,
    path_q: &str,
    notice: Option<&str>,
    error: Option<&str>,
    edit_path: Option<&str>,
    edit_content: Option<&str>,
) -> String {
    let crumbs = [
        ("Dashboard", Some("/dashboard")),
        ("Server", Some("/server")),
        ("Root File Manager", None),
    ];
    files_page(&FilesPageOpts {
        username,
        path_q,
        jail_root: Path::new("/"),
        base_url: "/server/files",
        op_url: "/server/files/op",
        upload_url: "/server/files/upload",
        query_extra: "",
        title: "Root File Manager",
        subtitle: "Browse and manage the server filesystem from CPN Panel.",
        brand_sub: "Root File Manager",
        risk_note: "Admin-only full filesystem access. Path traversal is blocked; protected system paths refuse delete/overwrite. Prefer site jails for routine hosting work.",
        crumbs: &crumbs,
        notice,
        error,
        edit_path,
        edit_content,
        domain: None,
    })
}

/// Site-jailed File Manager for one domain/subdomain home.
#[allow(clippy::too_many_arguments)]
pub fn site_files_page(
    username: &str,
    domain: &str,
    jail: &Path,
    path_q: &str,
    notice: Option<&str>,
    error: Option<&str>,
    edit_path: Option<&str>,
    edit_content: Option<&str>,
) -> String {
    let manage = format!("/websites/manage?domain={}", urlencoding_simple(domain));
    let crumbs = [
        ("Dashboard", Some("/dashboard")),
        ("Websites", Some("/websites")),
        ("Manage", Some(manage.as_str())),
        ("File Manager", None),
    ];
    let q = format!("domain={}&", urlencoding_simple(domain));
    let title = format!("File Manager: {domain}");
    let subtitle = format!(
        "Browse and manage files for {domain} (jailed to {}).",
        jail.display()
    );
    files_page(&FilesPageOpts {
        username,
        path_q,
        jail_root: jail,
        base_url: "/websites/files",
        op_url: "/websites/files/op",
        upload_url: "/websites/files/upload",
        query_extra: &q,
        title: &title,
        subtitle: &subtitle,
        brand_sub: "Site File Manager",
        risk_note: "Access is limited to this site home. Path traversal and sibling sites are blocked.",
        crumbs: &crumbs,
        notice,
        error,
        edit_path,
        edit_content,
        domain: Some(domain),
    })
}

fn edit_modal(opts: &FilesPageOpts<'_>, csrf: &str, cwd: &str) -> String {
    let Some(path) = opts.edit_path.filter(|p| !p.is_empty()) else {
        return String::new();
    };
    format!(
        r#"<div class="fm-modal" role="dialog" aria-modal="true" aria-label="Edit file">
  <form method="post" action="{op}" class="fm-modal-card">
    <h3>Edit {name}</h3>
    <input type="hidden" name="csrf" value="{csrf}">
    {domain}
    <input type="hidden" name="path" value="{cwd}">
    <input type="hidden" name="op" value="write">
    <input type="hidden" name="new_name" value="{path}">
    <textarea name="content" rows="18" class="fm-editor">{body}</textarea>
    <div class="fm-modal-actions">
      <button type="submit" class="btn-primary">Save</button>
      <a class="btn-secondary" href="{cancel}">Cancel</a>
    </div>
  </form>
</div>"#,
        op = html_escape(opts.op_url),
        name = html_escape(path),
        csrf = html_escape(csrf),
        domain = domain_hidden(opts.domain),
        cwd = html_escape(cwd),
        cancel = page_href(opts, cwd),
        path = html_escape(path),
        body = html_escape(opts.edit_content.unwrap_or("")),
    )
}

fn build_tree_html(opts: &FilesPageOpts<'_>, jail_home: &str, current: &str) -> String {
    let roots = match list_dir(opts.jail_root) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    let mut out = format!(
        r#"<ul class="fm-tree-list"><li><a href="{home}">{label}</a><ul>"#,
        home = page_href(opts, jail_home),
        label = html_escape(jail_home),
    );
    for ent in roots.into_iter().filter(|e| e.is_dir) {
        let p = opts
            .jail_root
            .join(&ent.basename)
            .display()
            .to_string()
            .replace('\\', "/");
        let open = current == p || current.starts_with(&(p.clone() + "/"));
        let kids = if open {
            match list_dir(Path::new(&p)) {
                Ok(entries) => {
                    let mut inner = String::from("<ul>");
                    for child in entries.into_iter().filter(|e| e.is_dir).take(40) {
                        let cp = format!("{}/{}", p.trim_end_matches('/'), child.basename);
                        inner.push_str(&format!(
                            r#"<li><a href="{href}">{name}</a></li>"#,
                            href = page_href(opts, &cp),
                            name = html_escape(&child.basename),
                        ));
                    }
                    inner.push_str("</ul>");
                    inner
                }
                Err(_) => String::new(),
            }
        } else {
            String::new()
        };
        out.push_str(&format!(
            r#"<li><a href="{href}">{name}</a>{kids}</li>"#,
            href = page_href(opts, &p),
            name = html_escape(&ent.basename),
            kids = kids,
        ));
    }
    out.push_str("</ul></li></ul>");
    out
}

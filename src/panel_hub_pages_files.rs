//! File Manager UI (classic hosting file manager layout, CPN branding).

use crate::panel_brand::brand_mark_svg;
use crate::panel_hub_http::urlencoding_simple;
use crate::panel_hub_pages_files_assets::{fm_asset_tags, fm_inline_boot};
use crate::panel_hub_pages_files_tree::build_tree_html;
use crate::panel_hubs::{feature_shell, notice_block};
use crate::panel_markdown::{
    MARKDOWN_PREVIEW_PATH, html_template_editor_field, markdown_editor_field_with_preview,
    markdown_toolbar_assets,
};
use crate::panel_ops_files::files_csrf_token;
use crate::panel_ops_path::resolve_under_jail;
use std::path::Path;

const SITE_READY_PREVIEW_PATH: &str = "/settings/site-messages/preview-site-ready";

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
    pub list_url: &'a str,
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

/// Fast HTML shell: no directory walk. Rows load from the JSON list endpoint.
pub fn files_page(opts: &FilesPageOpts<'_>) -> String {
    let csrf = files_csrf_token(opts.username);
    let home = opts.jail_root.display().to_string();
    let resolved = resolve_under_jail(opts.path_q, opts.jail_root);
    let body = match resolved {
        Ok(path) => {
            let path_s = path.display().to_string();
            let parent_href = path
                .parent()
                .filter(|p| resolve_under_jail(&p.display().to_string(), opts.jail_root).is_ok())
                .map(|p| page_href(opts, &p.display().to_string()))
                .unwrap_or_else(|| page_href(opts, &home));
            let tree = build_tree_html(opts.base_url, opts.query_extra, &home, &path_s);
            let editor = edit_modal(opts, &csrf, &path_s);
            let notices = format!(
                "{}{}",
                notice_block("ok", opts.notice),
                notice_block("error", opts.error),
            );
            let risk = format!(r#"<p class="muted">{}</p>"#, html_escape(opts.risk_note));
            format!(
                r#"{styles}
{risk}
{notices}
<div class="fm-root" id="fm-root" data-path="{path_esc}" data-csrf="{csrf}" data-base="{base}" data-op="{op}" data-upload="{upload}" data-list="{list}" data-qs="{qs}">
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
      <p id="fm-list-status" class="muted" hidden></p>
      <div class="table-wrap fm-table-wrap">
        <table class="data-table fm-table">
          <thead><tr><th></th><th>Name</th><th>Type</th><th>Size (KB)</th><th>Last Modified</th><th>Permissions</th></tr></thead>
          <tbody id="fm-rows"><tr><td colspan="6">Loading directory…</td></tr></tbody>
        </table>
      </div>
      <noscript><p class="panel-notice error">Enable JavaScript to list this directory. File operations stay on {op}.</p></noscript>
    </div>
  </div>
</div>
{editor}
{boot}"#,
                styles = fm_asset_tags(),
                risk = risk,
                notices = notices,
                path_esc = html_escape(&path_s),
                csrf = html_escape(&csrf),
                logo = brand_mark_svg(),
                brand = html_escape(opts.brand_sub),
                base = html_escape(opts.base_url),
                op = html_escape(opts.op_url),
                upload = html_escape(opts.upload_url),
                list = html_escape(opts.list_url),
                qs = html_escape(opts.query_extra),
                home_href = page_href(opts, &home),
                back = parent_href,
                refresh = page_href(opts, &path_s),
                domain_get = domain_hidden(opts.domain),
                domain_post = domain_hidden(opts.domain),
                tree = tree,
                editor = editor,
                boot = fm_inline_boot(),
            )
        }
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
        list_url: "/server/files/list",
        query_extra: "",
        title: "Root File Manager",
        subtitle: "Browse and manage the server filesystem from CPN Panel.",
        brand_sub: "Root File Manager",
        risk_note: "Admin-only full filesystem access. Path traversal is blocked; protected system paths refuse delete/overwrite. Prefer site jails for routine hosting work. The left tree is folders only; the table lists folders and files. Click a file name to edit UTF-8 text.",
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
        list_url: "/websites/files/list",
        query_extra: &q,
        title: &title,
        subtitle: &subtitle,
        brand_sub: "Site File Manager",
        risk_note: "Access is limited to this site home. Path traversal and sibling sites are blocked. The left tree is folders only; the table lists folders and files. Click a file name to edit UTF-8 text.",
        crumbs: &crumbs,
        notice,
        error,
        edit_path,
        edit_content,
        domain: Some(domain),
    })
}

fn path_looks_like_html(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm") || lower.ends_with(".xhtml")
}

fn edit_modal(opts: &FilesPageOpts<'_>, csrf: &str, cwd: &str) -> String {
    let Some(path) = opts.edit_path.filter(|p| !p.is_empty()) else {
        return String::new();
    };
    let body = opts.edit_content.unwrap_or("");
    let editor = if path_looks_like_html(path) {
        html_template_editor_field(
            "HTML source",
            "content",
            body,
            "<!DOCTYPE html>...",
            SITE_READY_PREVIEW_PATH,
        )
    } else {
        markdown_editor_field_with_preview(
            "File contents (Markdown toolbar, Preview, View HTML source)",
            "content",
            body,
            "Edit UTF-8 text…",
            MARKDOWN_PREVIEW_PATH,
        )
    };
    format!(
        r#"<div class="fm-modal" role="dialog" aria-modal="true" aria-label="Edit file">
  <form method="post" action="{op}" class="fm-modal-card fm-edit-card">
    <h3>Edit {name}</h3>
    <p class="muted fm-edit-hint">Text editor: UTF-8 only. Use Preview and View HTML source for Markdown/HTML. Binary files (databases, images, archives) cannot be edited here.</p>
    <input type="hidden" name="csrf" value="{csrf}">
    {domain}
    <input type="hidden" name="path" value="{cwd}">
    <input type="hidden" name="op" value="write">
    <input type="hidden" name="new_name" value="{path}">
    {editor}
    <div class="fm-modal-actions">
      <button type="submit" class="btn-primary">Save</button>
      <a class="btn-secondary" href="{cancel}">Cancel</a>
    </div>
  </form>
</div>
{assets}"#,
        op = html_escape(opts.op_url),
        name = html_escape(path),
        csrf = html_escape(csrf),
        domain = domain_hidden(opts.domain),
        cwd = html_escape(cwd),
        cancel = page_href(opts, cwd),
        path = html_escape(path),
        editor = editor,
        assets = markdown_toolbar_assets(),
    )
}

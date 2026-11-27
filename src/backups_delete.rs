//! Safe deletion of local backup archive files with confirmation UI helpers.
//!
//! Only deletes regular files under known backup roots (site/panel backups dirs
//! and documented restore upload locations). Never follows symlinks out of those
//! roots. Audit callers should log filename + directory only (no secrets).

use crate::backups::{BackupScope, resolve_archive_dir};
use crate::paths::{legacy_panel_backups_dir, panel_backups_dir};
use crate::sites::{list_sites, site_backups_dir};
use std::fs;
use std::path::{Path, PathBuf};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn is_safe_archive_name(name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() || name.len() > 255 {
        return false;
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".tar.gz")
        || lower.ends_with(".tgz")
        || lower.ends_with(".tar")
        || lower.ends_with(".zip")
        || lower.ends_with(".sql")
        || lower.ends_with(".sql.gz")
}

fn allowed_backup_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(panel_backups_dir());
    let custom = crate::panel_ops_backup_extra::effective_local_backup_dir();
    if custom != panel_backups_dir() {
        dirs.push(custom);
    }
    let legacy = legacy_panel_backups_dir();
    if legacy.is_dir() {
        dirs.push(legacy);
    }
    for path in [
        PathBuf::from("/home/cpn/backups"),
        PathBuf::from("/home/cpn-panel/backups"),
        PathBuf::from("/var/lib/cpn/backups"),
        PathBuf::from("/root"),
        PathBuf::from("/home"),
        PathBuf::from("/"),
    ] {
        if path.is_dir() {
            dirs.push(path);
        }
    }
    if let Ok(sites) = list_sites() {
        for site in sites {
            dirs.push(site_backups_dir(&site));
        }
    }
    let _ = resolve_archive_dir(BackupScope::Panel, "");
    dirs
}

fn is_broad_drop_root(root: &Path) -> bool {
    matches!(
        root.to_string_lossy().replace('\\', "/").as_str(),
        "/" | "/home" | "/root"
    )
}

fn path_under_allowed(path: &Path, allowed: &[PathBuf]) -> bool {
    let Ok(canon) = path.canonicalize() else {
        return false;
    };
    if !canon.is_file() {
        return false;
    }
    for root in allowed {
        let Ok(root_c) = root.canonicalize() else {
            continue;
        };
        if !canon.starts_with(&root_c) {
            continue;
        }
        if is_broad_drop_root(&root_c) {
            // Operator drop roots: only direct children (no recursive deletes).
            if canon
                .parent()
                .map(|p| p == root_c.as_path())
                .unwrap_or(false)
            {
                return true;
            }
            continue;
        }
        return true;
    }
    false
}

/// Delete one backup archive by basename under an explicit directory.
pub fn delete_backup_archive(dir: &str, name: &str) -> Result<String, String> {
    if !is_safe_archive_name(name) {
        return Err("Archive name is invalid.".into());
    }
    let dir = dir.trim();
    if dir.is_empty() {
        return Err("Archive directory is required.".into());
    }
    let candidate = PathBuf::from(dir).join(name.trim());
    let allowed = allowed_backup_dirs();
    // Allow delete when the parent exists under an allowed root, even before canonicalize.
    let parent = candidate
        .parent()
        .ok_or_else(|| "Archive path is invalid.".to_string())?;
    let parent_ok = allowed.iter().any(|root| {
        let Ok(root_c) = root.canonicalize() else {
            return parent == root.as_path();
        };
        match parent.canonicalize() {
            Ok(p) => p.starts_with(&root_c),
            Err(_) => parent == root.as_path() || parent.starts_with(root),
        }
    });
    if !parent_ok {
        return Err("Archive directory is outside allowed backup locations.".into());
    }
    if candidate.exists() {
        if candidate
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(true)
        {
            return Err("Refusing to delete a symlink.".into());
        }
        if !candidate.is_file() {
            return Err("Archive path is not a regular file.".into());
        }
        // When file exists, require canonicalize under allowed roots.
        if !path_under_allowed(&candidate, &allowed) {
            return Err("Archive is outside allowed backup locations.".into());
        }
    } else {
        return Err(format!("Archive `{}` was not found.", name.trim()));
    }
    fs::remove_file(&candidate).map_err(|e| format!("Could not delete archive: {e}"))?;
    Ok(format!("Deleted {}", name.trim()))
}

/// Per-row delete form (confirmation via shared modal script).
pub fn delete_archive_form_html(
    action: &str,
    scope: &str,
    domain: &str,
    dir: &str,
    name: &str,
    return_to: &str,
) -> String {
    format!(
        r#"<form method="post" action="{action}" class="stack-form" style="margin:0;" data-backup-delete-form data-backup-delete-name="{name}">
          <input type="hidden" name="scope" value="{scope}">
          <input type="hidden" name="domain" value="{domain}">
          <input type="hidden" name="dir" value="{dir}">
          <input type="hidden" name="archive" value="{name}">
          <input type="hidden" name="return_to" value="{ret}">
          <button type="submit" class="btn-secondary" style="color:#d92d20;border-color:#fecdca;">Delete</button>
        </form>"#,
        action = html_escape(action),
        scope = html_escape(scope),
        domain = html_escape(domain),
        dir = html_escape(dir),
        name = html_escape(name),
        ret = html_escape(return_to),
    )
}

/// Shared confirmation dialog for backup archive deletion.
pub fn delete_archive_modal_html() -> &'static str {
    r#"<dialog id="cpn-backup-delete-modal" class="cpn-modal" aria-labelledby="cpn-backup-delete-title" aria-describedby="cpn-backup-delete-desc">
  <div class="stack-form" style="padding:20px;margin:0;">
    <h2 id="cpn-backup-delete-title">Delete backup archive?</h2>
    <p id="cpn-backup-delete-desc" class="muted" style="margin:0 0 4px;">Delete <strong id="cpn-backup-delete-name"></strong>? This removes the local archive file only. Remote copies are not changed. This cannot be undone.</p>
    <div style="display:flex;gap:8px;justify-content:flex-end;margin-top:16px;">
      <button type="button" class="btn-secondary" id="cpn-backup-delete-cancel">Cancel</button>
      <button type="button" class="btn-primary" id="cpn-backup-delete-confirm" style="background:#d92d20;border-color:#d92d20;">Delete</button>
    </div>
  </div>
</dialog>
<script>
(function () {
  if (window.__cpnBackupDeleteBound) return;
  window.__cpnBackupDeleteBound = true;
  var pending = null;
  function modal() { return document.getElementById("cpn-backup-delete-modal"); }
  function nameOf(form) {
    var label = form.getAttribute("data-backup-delete-name") || "";
    return label.trim() || "this archive";
  }
  document.addEventListener("submit", function (ev) {
    var form = ev.target;
    if (!form || !form.hasAttribute || !form.hasAttribute("data-backup-delete-form")) return;
    if (form.getAttribute("data-backup-delete-confirmed") === "1") return;
    ev.preventDefault();
    var dlg = modal();
    var name = nameOf(form);
    if (dlg && typeof dlg.showModal === "function") {
      pending = form;
      var target = document.getElementById("cpn-backup-delete-name");
      if (target) target.textContent = name;
      dlg.showModal();
      var cancel = document.getElementById("cpn-backup-delete-cancel");
      if (cancel) cancel.focus();
      return;
    }
    if (window.confirm("Delete backup archive " + name + "? This cannot be undone.")) {
      form.setAttribute("data-backup-delete-confirmed", "1");
      form.submit();
    }
  }, true);
  document.addEventListener("click", function (ev) {
    var t = ev.target;
    if (!t || !t.id) return;
    var dlg = modal();
    if (t.id === "cpn-backup-delete-cancel") {
      if (dlg) dlg.close();
      pending = null;
      return;
    }
    if (t.id === "cpn-backup-delete-confirm" && pending) {
      var form = pending;
      pending = null;
      if (dlg) dlg.close();
      form.setAttribute("data-backup-delete-confirmed", "1");
      form.submit();
    }
  });
})();
</script>"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_names() {
        assert!(!is_safe_archive_name("../x.tar.gz"));
        assert!(!is_safe_archive_name("a/b.tar.gz"));
        assert!(is_safe_archive_name("panel-123.tar.gz"));
        assert!(is_safe_archive_name("site-example.com-1.zip"));
    }
}

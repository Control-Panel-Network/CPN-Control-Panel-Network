//! Format-specific restore apply helpers (WordPress, cPanel, classic source meta.xml, CPN).

use crate::backup_restore_extract::copy_tree;
use crate::panel_ops_db::{
    create_database, list_databases, local_mariadb_ready, mariadb_client_bin,
};
use crate::sites::SiteRecord;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Selective apply flags for multi-entity restore.
#[derive(Debug, Clone)]
pub struct RestoreApplyOpts {
    pub restore_website: bool,
    pub restore_plugins: bool,
    /// `None` = all SQL dumps; `Some(names)` = only matching basenames (empty = skip).
    pub database_names: Option<BTreeSet<String>>,
    pub include_email: bool,
    pub include_docker: bool,
    pub include_dns: bool,
    pub include_panel_config: bool,
}

impl Default for RestoreApplyOpts {
    fn default() -> Self {
        Self {
            restore_website: true,
            restore_plugins: true,
            database_names: None,
            include_email: false,
            include_docker: false,
            include_dns: false,
            include_panel_config: false,
        }
    }
}

fn sql_allowed(path: &Path, filter: &Option<BTreeSet<String>>) -> bool {
    let Some(names) = filter else {
        return true;
    };
    if names.is_empty() {
        return false;
    }
    let file = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let stem = file
        .trim_end_matches(".sql.gz")
        .trim_end_matches(".sql")
        .trim_end_matches(".gz");
    let stem_clean: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let stem_clean = stem_clean.trim_matches('_').to_string();
    names.contains(&stem_clean) || names.contains(&file) || names.contains(&stem.to_string())
}

pub(crate) fn find_dir_named(root: &Path, name: &str) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }
    if root.file_name().and_then(|v| v.to_str()) == Some(name) {
        return Some(root.to_path_buf());
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                if ent.file_name().to_string_lossy() == name {
                    return Some(path);
                }
                if path.components().count() < root.components().count() + 6 {
                    stack.push(path);
                }
            }
        }
    }
    None
}

pub(crate) fn find_file_named(root: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_file() && ent.file_name().to_string_lossy().eq_ignore_ascii_case(name) {
                return Some(path);
            }
            if path.is_dir() && path.components().count() < root.components().count() + 8 {
                stack.push(path);
            }
        }
    }
    None
}

pub(crate) fn collect_sql_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_file() {
                let name = ent.file_name().to_string_lossy().to_ascii_lowercase();
                if name.ends_with(".sql") || name.ends_with(".sql.gz") {
                    out.push(path);
                }
            } else if path.is_dir() && path.components().count() < root.components().count() + 6 {
                stack.push(path);
            }
        }
    }
    out.sort();
    out
}

pub(crate) fn restore_docroot_from(src: &Path, site: &SiteRecord) -> Result<(), String> {
    let dest = PathBuf::from(&site.docroot);
    if !dest.exists() {
        fs::create_dir_all(&dest).map_err(|e| format!("docroot mkdir: {e}"))?;
    }
    if src.is_dir() {
        for ent in fs::read_dir(src).map_err(|e| format!("read src: {e}"))? {
            let ent = ent.map_err(|e| e.to_string())?;
            let from = ent.path();
            let to = dest.join(ent.file_name());
            if from.is_dir() {
                let _ = fs::remove_dir_all(&to);
                copy_tree(&from, &to)?;
            } else if from.is_file() {
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
            }
        }
    }
    Ok(())
}

pub(crate) fn restore_cpn(
    staging: &Path,
    site: &SiteRecord,
    opts: &RestoreApplyOpts,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if opts.restore_website {
        if let Some(pub_html) = find_dir_named(staging, "public_html") {
            restore_docroot_from(&pub_html, site)?;
        } else {
            warnings.push("CPN archive had no public_html/; skipped website files.".into());
        }
    } else {
        warnings.push("Skipped website files (not selected).".into());
    }
    if opts.restore_plugins {
        if let Some(plugins) = find_dir_named(staging, "plugins") {
            let home = Path::new(&site.docroot)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("/home").join(&site.domain));
            let dest = home.join("plugins");
            let _ = fs::remove_dir_all(&dest);
            copy_tree(&plugins, &dest)?;
        }
    }
    if let Some(sql) = find_file_named(staging, "databases.sql") {
        if sql_allowed(&sql, &opts.database_names) {
            import_sql_best_effort(&sql, None, warnings)?;
        } else {
            warnings.push("Skipped databases.sql (not selected).".into());
        }
    }
    if find_dir_named(staging, "panel-config").is_some() {
        if opts.include_panel_config {
            warnings.push(
                "Panel-config was selected but not applied to live panel data (avoids wiping MFA keys and sessions). Use a dedicated panel recovery path if needed."
                    .into(),
            );
        } else {
            warnings.push(
                "Panel-config payload was present but not selected (site restore only)."
                    .into(),
            );
        }
    }
    Ok(())
}

pub(crate) fn restore_wordpress(
    staging: &Path,
    site: &SiteRecord,
    db_name_override: &str,
    opts: &RestoreApplyOpts,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let wp_root = if find_file_named(staging, "wp-config.php").is_some()
        && staging.join("wp-content").is_dir()
    {
        staging.to_path_buf()
    } else if let Some(cfg) = find_file_named(staging, "wp-config.php") {
        cfg.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| staging.to_path_buf())
    } else if let Some(content) = find_dir_named(staging, "wp-content") {
        content
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| staging.to_path_buf())
    } else if let Some(pub_html) = find_dir_named(staging, "public_html") {
        pub_html
    } else {
        staging.to_path_buf()
    };

    if opts.restore_website {
        restore_docroot_from(&wp_root, site)?;
    } else {
        warnings.push("Skipped WordPress website files (not selected).".into());
    }

    let sql_files: Vec<PathBuf> = collect_sql_files(staging)
        .into_iter()
        .filter(|p| sql_allowed(p, &opts.database_names))
        .collect();
    let db_name = if !db_name_override.is_empty() {
        Some(db_name_override.to_string())
    } else {
        guess_db_name_from_wp_config(&PathBuf::from(&site.docroot).join("wp-config.php"))
    };

    if opts.database_names.as_ref().is_some_and(|s| s.is_empty()) {
        warnings.push("Skipped SQL import (no databases selected).".into());
        return Ok(());
    }

    if let Some(name) = db_name.as_ref() {
        match create_database(name) {
            Ok(msg) => warnings.push(msg),
            Err(err) => warnings.push(format!("Could not create database `{name}`: {err}")),
        }
    }

    if sql_files.is_empty() {
        warnings.push("No selected .sql dump found in the WordPress archive.".into());
    } else {
        for sql in &sql_files {
            import_sql_best_effort(sql, db_name.as_deref(), warnings)?;
        }
    }

    if opts.restore_website {
        if let Some(name) = db_name.as_ref() {
            remap_wp_config_db(
                &PathBuf::from(&site.docroot).join("wp-config.php"),
                name,
                warnings,
            )?;
        } else {
            warnings.push(
                "Could not determine DB name for wp-config remap. Set Target DB name on restore if needed."
                    .into(),
            );
        }
    }
    Ok(())
}

pub(crate) fn restore_cpanel(
    staging: &Path,
    site: &SiteRecord,
    opts: &RestoreApplyOpts,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if opts.restore_website {
        let src = if let Some(homedir) = find_dir_named(staging, "homedir") {
            let pub_html = homedir.join("public_html");
            if pub_html.is_dir() { pub_html } else { homedir }
        } else if let Some(pub_html) = find_dir_named(staging, "public_html") {
            pub_html
        } else {
            return Err("cPanel archive missing homedir/public_html.".into());
        };
        restore_docroot_from(&src, site)?;
    } else {
        warnings.push("Skipped cPanel website files (not selected).".into());
    }

    let mysql_dir = find_dir_named(staging, "mysql");
    let mut sqls = Vec::new();
    if let Some(dir) = mysql_dir {
        sqls.extend(collect_sql_files(&dir));
    }
    sqls.extend(collect_sql_files(staging).into_iter().filter(|p| {
        let s = p.to_string_lossy().to_ascii_lowercase();
        s.contains("/mysql/") || s.ends_with("mysql.sql")
    }));
    sqls.sort();
    sqls.dedup();
    sqls.retain(|p| sql_allowed(p, &opts.database_names));
    if sqls.is_empty() {
        warnings.push(
            "No selected SQL dumps found under the cPanel `mysql/` folder (compatibility path; imports run against MariaDB)."
                .into(),
        );
    } else {
        for sql in sqls {
            import_sql_best_effort(&sql, None, warnings)?;
        }
    }
    if opts.include_email {
        warnings.push(
            "cPanel email was selected; account recreation from the archive is best-effort only (files + DB applied)."
                .into(),
        );
    }
    if opts.include_dns {
        warnings.push(
            "cPanel DNS/zone data was selected; automatic DNS publish is not applied yet (confirm zones manually)."
                .into(),
        );
    }
    if !opts.include_email && !opts.include_dns {
        warnings.push(
            "cPanel email / DNS / reseller metadata is not fully imported (files + DB best-effort)."
                .into(),
        );
    }
    Ok(())
}

pub(crate) fn restore_cyberpanel(
    staging: &Path,
    site: &SiteRecord,
    opts: &RestoreApplyOpts,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if opts.restore_website {
        let src = find_dir_named(staging, "public_html").ok_or_else(|| {
            "Source control-panel archive missing public_html/ (expected classic meta.xml backup)."
                .to_string()
        })?;
        let nested = src.join("public_html");
        let files_src = if nested.is_dir() { nested } else { src };
        restore_docroot_from(&files_src, site)?;
    } else {
        warnings.push("Skipped website files (not selected).".into());
    }

    let sqls: Vec<PathBuf> = collect_sql_files(staging)
        .into_iter()
        .filter(|p| sql_allowed(p, &opts.database_names))
        .collect();
    if sqls.is_empty() {
        warnings.push("No selected database .sql files found in source control-panel archive.".into());
    } else {
        for sql in sqls {
            // Filename-based DB create/USE happens inside import_sql_best_effort.
            import_sql_best_effort(&sql, None, warnings)?;
        }
    }
    if find_dir_named(staging, "vmail").is_some() {
        if opts.include_email {
            warnings.push(
                "vmail/ email data was selected; mailbox import is best-effort and may be skipped on this host."
                    .into(),
            );
        } else {
            warnings.push(
                "vmail/ email data was present but not selected.".into(),
            );
        }
    }
    if opts.include_docker {
        warnings.push(
            "Docker/compose payload was selected; stacks are not auto-recreated from this archive yet."
                .into(),
        );
    }
    if opts.include_dns {
        warnings.push(
            "DNS/zone payload was selected; automatic Cloudflare/DNS publish is not applied yet."
                .into(),
        );
    }
    if find_file_named(staging, "meta.xml").is_some() {
        warnings.push(
            "meta.xml was detected (source control-panel format). Site/email account recreation from meta is not applied yet; use Create domain if missing for the primary site."
                .into(),
        );
    }
    Ok(())
}

pub(crate) fn guess_db_name_from_wp_config(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("define")
            && t.contains("DB_NAME")
            && let Some(start) = t.find(',')
        {
            let rest = &t[start + 1..];
            let rest = rest.trim().trim_start_matches(['\'', '"']);
            let end = rest.find(['\'', '"', ')']).unwrap_or(rest.len());
            let name = rest[..end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn remap_wp_config_db(
    path: &Path,
    db_name: &str,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if !path.is_file() {
        warnings.push("wp-config.php missing after file restore; skipped credential remap.".into());
        return Ok(());
    }
    let raw = fs::read_to_string(path).map_err(|e| format!("read wp-config: {e}"))?;
    let mut out = String::with_capacity(raw.len() + 64);
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("define") && trimmed.contains("DB_NAME") {
            out.push_str(&format!(
                "define( 'DB_NAME', '{}' );\n",
                db_name.replace('\'', "")
            ));
        } else if trimmed.starts_with("define") && trimmed.contains("DB_HOST") {
            out.push_str("define( 'DB_HOST', 'localhost' );\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    fs::write(path, out).map_err(|e| format!("write wp-config: {e}"))?;
    warnings.push(format!(
        "Remapped wp-config.php DB_NAME to `{db_name}` and DB_HOST to localhost. Update DB_USER/DB_PASSWORD to a local MariaDB user if needed."
    ));
    Ok(())
}

/// Infer a MariaDB database name from a dump filename (`news_disco.sql` -> `news_disco`).
fn guess_db_name_from_sql_filename(name: &str) -> Option<String> {
    let base = name
        .trim()
        .trim_end_matches(".sql.gz")
        .trim_end_matches(".sql")
        .trim_end_matches(".gz");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('_');
    if cleaned.is_empty() || cleaned == "dump" || cleaned == "database" || cleaned == "db" {
        return None;
    }
    // Classic source-panel dumps often look like `news_cms.sql` / `prefix_dbname.sql`.
    Some(cleaned.to_string())
}

fn import_sql_best_effort(
    sql_path: &Path,
    prefer_db: Option<&str>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if !local_mariadb_ready() {
        warnings.push(format!(
            "Skipped SQL import `{}`: no local MariaDB detected. Install MariaDB from Host packages (CPN does not offer Oracle MySQL as a host database).",
            sql_path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("dump.sql")
        ));
        return Ok(());
    }
    let bin = mariadb_client_bin().ok_or_else(|| {
        "MariaDB client not found (`mariadb` or `mysql`). Install MariaDB client tools.".to_string()
    })?;

    let name = sql_path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("dump.sql")
        .to_string();

    // Classic meta.xml archives dump one .sql per database with no USE statement.
    // Prefer an explicit target DB, else infer from the dump filename.
    let inferred = guess_db_name_from_sql_filename(&name);
    let target_db = prefer_db
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or(inferred);

    if let Some(ref dbn) = target_db {
        match create_database(dbn) {
            Ok(_) => {}
            Err(e) => warnings.push(format!(
                "Could not ensure database `{dbn}` before importing `{name}`: {e}"
            )),
        }
    } else {
        warnings.push(format!(
            "SQL `{name}` has no target database name; import may fail with \"No database selected\"."
        ));
    }

    let mut cmd = Command::new(bin);
    if let Some(ref dbn) = target_db {
        cmd.arg(dbn);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start {bin}: {e}"))?;

    let data = if name.ends_with(".gz") {
        let out = Command::new("gzip")
            .args(["-dc"])
            .arg(sql_path)
            .output()
            .map_err(|e| format!("gzip decompress failed: {e}"))?;
        if !out.status.success() {
            warnings.push(format!("Failed to decompress `{name}`; skipped."));
            let _ = child.kill();
            return Ok(());
        }
        out.stdout
    } else {
        fs::read(sql_path).map_err(|e| format!("read sql: {e}"))?
    };

    // Prefix USE when we have a target DB and the dump never selects one.
    let mut payload = Vec::with_capacity(data.len() + 64);
    if let Some(ref dbn) = target_db {
        payload.extend_from_slice(format!("USE `{dbn}`;\n").as_bytes());
    }
    payload.extend_from_slice(&data);

    let mut write_err: Option<String> = None;
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(&payload) {
            // Client often exits early (auth / no DB); broken pipe must not abort file restore.
            write_err = Some(format!("write sql stdin: {e}"));
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("wait for {bin}: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_trim = stderr.trim();
    if output.status.success() {
        let into = target_db
            .as_deref()
            .map(|d| format!(" into `{d}`"))
            .unwrap_or_default();
        warnings.push(format!("Imported SQL `{name}`{into} via MariaDB."));
        let _ = list_databases();
    } else {
        let detail = if stderr_trim.is_empty() {
            write_err.unwrap_or_else(|| "check dump format and local MariaDB auth".into())
        } else {
            stderr_trim.chars().take(240).collect::<String>()
        };
        warnings.push(format!("SQL import of `{name}` failed ({detail})."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wp_db_name() {
        let dir = std::env::temp_dir().join(format!("cpn-wp-cfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("wp-config.php");
        fs::write(
            &cfg,
            "<?php\ndefine('DB_NAME', 'my_wp_db');\ndefine('DB_HOST', '127.0.0.1');\n",
        )
        .unwrap();
        assert_eq!(
            guess_db_name_from_wp_config(&cfg).as_deref(),
            Some("my_wp_db")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn guesses_db_from_classic_sql_filename() {
        assert_eq!(
            guess_db_name_from_sql_filename("news_disco.sql").as_deref(),
            Some("news_disco")
        );
        assert_eq!(
            guess_db_name_from_sql_filename("news_cms.sql.gz").as_deref(),
            Some("news_cms")
        );
        assert!(guess_db_name_from_sql_filename("dump.sql").is_none());
    }

    #[test]
    fn finds_cpanel_mysql_dump_layout() {
        let dir = std::env::temp_dir().join(format!("cpn-cpanel-mysql-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mysql = dir.join("mysql");
        fs::create_dir_all(&mysql).unwrap();
        let sql = mysql.join("user_wp.sql");
        fs::write(&sql, "CREATE TABLE t (id INT);\n").unwrap();
        assert_eq!(
            find_dir_named(&dir, "mysql").as_deref(),
            Some(mysql.as_path())
        );
        let collected = collect_sql_files(&mysql);
        assert_eq!(collected.len(), 1);
        assert!(collected[0].ends_with("user_wp.sql"));
        let _ = fs::remove_dir_all(&dir);
    }
}

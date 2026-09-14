//! Format-specific restore apply helpers (WordPress, cPanel, CyberPanel source, CPN).

use crate::backup_restore_extract::copy_tree;
use crate::panel_ops_db::{create_database, list_databases, local_mariadb_ready, mariadb_client_bin};
use crate::sites::SiteRecord;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if let Some(pub_html) = find_dir_named(staging, "public_html") {
        restore_docroot_from(&pub_html, site)?;
    } else {
        warnings.push("CPN archive had no public_html/; skipped website files.".into());
    }
    if let Some(plugins) = find_dir_named(staging, "plugins") {
        let home = Path::new(&site.docroot)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/home").join(&site.domain));
        let dest = home.join("plugins");
        let _ = fs::remove_dir_all(&dest);
        copy_tree(&plugins, &dest)?;
    }
    if let Some(sql) = find_file_named(staging, "databases.sql") {
        import_sql_best_effort(&sql, None, warnings)?;
    }
    if find_dir_named(staging, "panel-config").is_some() {
        warnings.push(
            "Panel-config payload was present but not applied (site restore only). Use a panel-scoped restore path for panel data."
                .into(),
        );
    }
    Ok(())
}

pub(crate) fn restore_wordpress(
    staging: &Path,
    site: &SiteRecord,
    db_name_override: &str,
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

    restore_docroot_from(&wp_root, site)?;

    let sql_files = collect_sql_files(staging);
    let db_name = if !db_name_override.is_empty() {
        Some(db_name_override.to_string())
    } else {
        guess_db_name_from_wp_config(&PathBuf::from(&site.docroot).join("wp-config.php"))
    };

    if let Some(name) = db_name.as_ref() {
        match create_database(name) {
            Ok(msg) => warnings.push(msg),
            Err(err) => warnings.push(format!("Could not create database `{name}`: {err}")),
        }
    }

    if sql_files.is_empty() {
        warnings.push("No .sql dump found in the WordPress archive.".into());
    } else {
        for sql in &sql_files {
            import_sql_best_effort(sql, db_name.as_deref(), warnings)?;
        }
    }

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
    Ok(())
}

pub(crate) fn restore_cpanel(
    staging: &Path,
    site: &SiteRecord,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let src = if let Some(homedir) = find_dir_named(staging, "homedir") {
        let pub_html = homedir.join("public_html");
        if pub_html.is_dir() { pub_html } else { homedir }
    } else if let Some(pub_html) = find_dir_named(staging, "public_html") {
        pub_html
    } else {
        return Err("cPanel archive missing homedir/public_html.".into());
    };
    restore_docroot_from(&src, site)?;

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
    if sqls.is_empty() {
        warnings.push(
            "No SQL dumps found under the cPanel `mysql/` folder (compatibility path; imports run against MariaDB)."
                .into(),
        );
    } else {
        for sql in sqls {
            import_sql_best_effort(&sql, None, warnings)?;
        }
    }
    warnings.push(
        "cPanel email / DNS / reseller metadata is not fully imported (files + DB best-effort)."
            .into(),
    );
    Ok(())
}

pub(crate) fn restore_cyberpanel(
    staging: &Path,
    site: &SiteRecord,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let src = find_dir_named(staging, "public_html").ok_or_else(|| {
        "CyberPanel archive missing public_html/ (expected classic meta.xml backup).".to_string()
    })?;
    let nested = src.join("public_html");
    let files_src = if nested.is_dir() { nested } else { src };
    restore_docroot_from(&files_src, site)?;

    let sqls = collect_sql_files(staging);
    if sqls.is_empty() {
        warnings.push("No database .sql files found in CyberPanel archive.".into());
    } else {
        for sql in sqls {
            import_sql_best_effort(&sql, None, warnings)?;
        }
    }
    if find_dir_named(staging, "vmail").is_some() {
        warnings.push(
            "vmail/ email data was present but not imported (best-effort: files/DB only).".into(),
        );
    }
    if find_file_named(staging, "meta.xml").is_some() {
        warnings.push(
            "meta.xml was detected (CyberPanel source format). Site/email account recreation from meta is not applied."
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
        "MariaDB client not found (`mariadb` or `mysql`). Install MariaDB client tools."
            .to_string()
    })?;

    let name = sql_path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("dump.sql")
        .to_string();

    if let Some(dbn) = prefer_db {
        let _ = create_database(dbn);
    }

    let mut cmd = Command::new(bin);
    if let Some(dbn) = prefer_db {
        cmd.arg(dbn);
    }
    cmd.stdin(Stdio::piped());
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

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&data)
            .map_err(|e| format!("write sql stdin: {e}"))?;
    }
    let status = child.wait().map_err(|e| format!("wait for {bin}: {e}"))?;
    if status.success() {
        warnings.push(format!("Imported SQL `{name}` into MariaDB."));
        let _ = list_databases();
    } else {
        warnings.push(format!(
            "SQL import of `{name}` failed (check dump format and local MariaDB auth)."
        ));
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

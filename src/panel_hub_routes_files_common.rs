//! Shared helpers for Root and Site File Manager HTTP routes.

use crate::panel_hub_http::redirect_notice;
use crate::panel_hub_http::urlencoding_simple;
use crate::panel_ops_files::{
    copy_entries, create_file, delete_names, mkdir, move_entries, rename_entry, write_text,
};
use crate::panel_ops_files_archive::{compress_entries, extract_entry};
use actix_web::{HttpRequest, HttpResponse};
use std::collections::HashMap;
use std::path::Path;

pub(crate) fn same_origin_ok(http: &HttpRequest) -> bool {
    let Some(origin) = http
        .headers()
        .get("origin")
        .or_else(|| http.headers().get("referer"))
        .and_then(|v| v.to_str().ok())
    else {
        return true;
    };
    let host = http.connection_info().host().to_string();
    origin.contains(&host)
}

pub(crate) fn root_redirect(path: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    let base = format!(
        "/server/files?path={}",
        urlencoding_simple(if path.trim().is_empty() { "/" } else { path })
    );
    redirect_notice(&base, notice, error)
}

pub(crate) fn site_redirect(
    domain: &str,
    path: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> HttpResponse {
    let base = format!(
        "/websites/files?domain={}&path={}",
        urlencoding_simple(domain),
        urlencoding_simple(if path.trim().is_empty() { "/" } else { path })
    );
    redirect_notice(&base, notice, error)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_op(
    op: &str,
    path: &str,
    names: &[String],
    new_name: &str,
    dest: &str,
    archive_name: &str,
    content: &str,
    jail: &Path,
) -> Result<String, String> {
    match op {
        "mkdir" => mkdir(path, new_name, jail),
        "create" => create_file(path, new_name, jail),
        "delete" => delete_names(path, names, jail),
        "rename" => {
            let from = names.first().map(String::as_str).unwrap_or("");
            rename_entry(path, from, new_name, jail)
        }
        "copy" => copy_entries(path, names, dest, jail),
        "move" => move_entries(path, names, dest, jail),
        "write" => write_text(new_name, content, jail),
        "compress" => compress_entries(path, names, archive_name, jail),
        "extract" => {
            let archive = names.first().map(String::as_str).unwrap_or("");
            extract_entry(path, archive, jail)
        }
        _ => Err("Unknown file operation".into()),
    }
}

pub(crate) fn parse_op_form(
    form: &HashMap<String, String>,
) -> (String, String, String, String, String, String, Vec<String>) {
    let path = form
        .get("path")
        .map(String::as_str)
        .unwrap_or("/")
        .to_string();
    let op = form
        .get("op")
        .map(String::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let new_name = form
        .get("new_name")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let dest = form
        .get("dest")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let archive_name = form
        .get("archive_name")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let content = form.get("content").cloned().unwrap_or_default();
    let names: Vec<String> = form
        .get("names_csv")
        .map(|v| {
            v.lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    (path, op, new_name, dest, archive_name, content, names)
}

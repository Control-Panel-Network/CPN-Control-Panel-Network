use crate::model::MailReleaseInfo;
use std::process::Stdio;
use tokio::process::Command;

fn curated_fallbacks() -> Vec<MailReleaseInfo> {
    vec![
        MailReleaseInfo {
            id: "snappymail".into(),
            label: "SnappyMail".into(),
            version: "2.38.2".into(),
            released_on: "2024-10-09".into(),
        },
        MailReleaseInfo {
            id: "roundcube".into(),
            label: "Roundcube".into(),
            version: "1.7.3".into(),
            released_on: "2026-08-09".into(),
        },
        MailReleaseInfo {
            id: "thunderbird".into(),
            label: "Thunderbird".into(),
            version: "155.0".into(),
            released_on: "2026-09-01".into(),
        },
    ]
}

async fn curl_json(url: &str) -> Option<String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "8",
            "-H",
            "Accept: application/json",
            "-H",
            "User-Agent: cpn-installer",
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let body = String::from_utf8(output.stdout).ok()?;
    if body.trim().is_empty() {
        return None;
    }
    Some(body)
}

fn parse_github_release(body: &str) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let tag = value.get("tag_name")?.as_str()?.trim();
    let version = tag.trim_start_matches('v').to_string();
    if version.is_empty() {
        return None;
    }
    let published = value
        .get("published_at")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .chars()
        .take(10)
        .collect::<String>();
    if published.len() != 10 {
        return None;
    }
    Some((version, published))
}

fn parse_thunderbird_release(body: &str) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let latest = value
        .get("LATEST_THUNDERBIRD_VERSION")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())?
        .to_string();
    let history = value.get("THUNDERBIRD_HISTORY")?.as_object()?;
    let released_on = history
        .get(&latest)
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| item.len() >= 10)
        .map(|item| item.chars().take(10).collect::<String>())
        .unwrap_or_else(|| "2026-09-01".into());
    Some((latest, released_on))
}

async fn refresh_one(id: &str, label: &str, fallback: &MailReleaseInfo) -> MailReleaseInfo {
    let refreshed = match id {
        "snappymail" => {
            let body =
                curl_json("https://api.github.com/repos/the-djmaze/snappymail/releases/latest")
                    .await;
            body.as_deref().and_then(parse_github_release)
        }
        "roundcube" => {
            let body =
                curl_json("https://api.github.com/repos/roundcube/roundcubemail/releases/latest")
                    .await;
            body.as_deref().and_then(parse_github_release)
        }
        "thunderbird" => {
            let body =
                curl_json("https://product-details.mozilla.org/1.0/thunderbird_versions.json")
                    .await;
            body.as_deref().and_then(parse_thunderbird_release)
        }
        _ => None,
    };
    match refreshed {
        Some((version, released_on)) => MailReleaseInfo {
            id: id.into(),
            label: label.into(),
            version,
            released_on,
        },
        None => fallback.clone(),
    }
}

/// Load curated mail release metadata, optionally refreshed via curl at startup.
pub async fn load_mail_releases() -> Vec<MailReleaseInfo> {
    let fallbacks = curated_fallbacks();
    let mut releases = Vec::with_capacity(fallbacks.len());
    for fallback in &fallbacks {
        releases.push(refresh_one(&fallback.id, &fallback.label, fallback).await);
    }
    releases
}

//! HTML + plain bodies for CPN Panel feedback mail (no secrets).

use crate::http_helpers::VERSION;

/// CID for the inline CPN brand mark PNG (`multipart/related`).
pub const FEEDBACK_LOGO_CID: &str = "cpn-logo@cpn";

/// Panel brand mark PNG used in Feedback HTML (same asset as `/cpn-brand-mark` family).
pub fn feedback_logo_png() -> &'static [u8] {
    include_bytes!("../installer-ui/src/assets/cpn-brand-mark.png")
}

pub(crate) fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub struct FeedbackMailInput<'a> {
    pub username: &'a str,
    pub sender_email: &'a str,
    pub category: &'a str,
    pub subject: &'a str,
    pub message: &'a str,
    pub host: &'a str,
    pub sent_at_unix: u64,
}

/// UTC `dd/mm/yyyy HH:MM` (24-hour).
pub fn format_eu_datetime_utc(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let rem = unix % 86_400;
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let (year, month, day) = civil_from_days(days);
    format!("{day:02}/{month:02}/{year} {hour:02}:{minute:02} UTC")
}

/// Days since Unix epoch (1970-01-01) to Gregorian Y-M-D (Howard Hinnant).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn category_pill_colors(category: &str) -> (&'static str, &'static str) {
    match category {
        "Bug" => ("#fee2e2", "#b91c1c"),
        "Feature request" => ("#e8f1ff", "#006CFA"),
        "Usability" => ("#d1fae5", "#047857"),
        _ => ("#e2e8f0", "#334155"),
    }
}

pub fn feedback_plain_body(input: &FeedbackMailInput<'_>) -> String {
    let sent = format_eu_datetime_utc(input.sent_at_unix);
    format!(
        "CPN Panel feedback\r\n\r\n\
Category: {category}\r\n\
Subject: {subject}\r\n\
User: {username}\r\n\
User email: {sender_email}\r\n\
Panel host: {host}\r\n\
Panel version: {version}\r\n\
Time: {sent}\r\n\r\n\
Message:\r\n{message}\r\n",
        category = input.category,
        subject = input.subject,
        username = input.username,
        sender_email = input.sender_email,
        host = input.host,
        version = VERSION,
        sent = sent,
        message = input.message,
    )
}

fn nl2br_escaped(value: &str) -> String {
    html_escape(value).lines().collect::<Vec<_>>().join("<br>")
}

fn meta_row(label: &str, value: &str) -> String {
    format!(
        "<tr>\
<td style=\"padding:6px 12px 6px 0;color:#64748b;font-size:13px;width:132px;vertical-align:top;\">{label}</td>\
<td style=\"padding:6px 0;color:#161F2B;font-size:13px;overflow-wrap:anywhere;\">{value}</td>\
</tr>",
        label = html_escape(label),
        value = html_escape(value),
    )
}

pub fn feedback_html_body(input: &FeedbackMailInput<'_>) -> String {
    let (pill_bg, pill_fg) = category_pill_colors(input.category);
    let sent = format_eu_datetime_utc(input.sent_at_unix);
    let subject = html_escape(input.subject);
    let category = html_escape(input.category);
    let message = nl2br_escaped(input.message);
    let meta = format!(
        "{}{}{}{}{}",
        meta_row("User", input.username),
        meta_row("User email", input.sender_email),
        meta_row("Panel host", input.host),
        meta_row("Panel version", VERSION),
        meta_row("Time", &sent),
    );
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>CPN Panel feedback</title>
</head>
<body style="margin:0;padding:0;background-color:#0b1220;color:#161F2B;font-family:'Segoe UI',Helvetica,Arial,sans-serif;">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background-color:#0b1220;background:#0b1220;padding:28px 12px;">
<tr><td align="center">
<table role="presentation" width="600" cellspacing="0" cellpadding="0" style="max-width:600px;width:100%;background-color:#ffffff;background:#ffffff;border:1px solid #1e3a5f;">
<tr>
<td style="background-color:#161F2B;background:#161F2B;padding:22px 24px;">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0">
<tr>
<td width="48" valign="middle" style="width:48px;">
<img src="cid:cpn-logo@cpn" alt="CPN Control Panel Network" width="40" height="40" style="display:block;width:40px;height:40px;border:0;outline:none;text-decoration:none;border-radius:8px;">
</td>
<td valign="middle" style="padding-left:12px;">
<div style="font-size:20px;line-height:1.2;font-weight:800;color:#ffffff;">CPN Panel</div>
<div style="font-size:12px;color:#9db4d0;padding-top:4px;">Operator feedback</div>
</td>
</tr>
</table>
</td>
</tr>
<tr><td style="height:8px;line-height:8px;font-size:0;background-color:#006CFA;background:#006CFA;">&nbsp;</td></tr>
<tr>
<td style="padding:24px 24px 10px 24px;">
<span style="display:inline-block;padding:5px 12px;border-radius:999px;background-color:{pill_bg};color:{pill_fg};font-size:12px;font-weight:800;letter-spacing:0.04em;">{category}</span>
<h1 style="margin:14px 0 18px 0;font-size:24px;line-height:1.25;color:#0b1220;font-weight:800;">{subject}</h1>
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="border-collapse:collapse;">
<tr>
<td style="width:8px;background-color:#006CFA;background:#006CFA;font-size:0;line-height:0;">&nbsp;</td>
<td style="padding:18px 16px;background-color:#f4f7fb;background:#f4f7fb;color:#0f172a;font-size:16px;line-height:1.6;">{message}</td>
</tr>
</table>
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="margin-top:20px;">{meta}</table>
</td>
</tr>
<tr>
<td style="padding:14px 24px 22px 24px;background-color:#eef3f9;background:#eef3f9;color:#334155;font-size:12px;line-height:1.5;">Sent from CPN Panel as an HTML email. If you only see a plain text block, refresh or switch the client to HTML view.</td>
</tr>
</table>
</td></tr>
</table>
</body>
</html>"#
    )
}

pub fn build_feedback_mail(input: &FeedbackMailInput<'_>) -> (String, String) {
    (feedback_plain_body(input), feedback_html_body(input))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(message: &str) -> FeedbackMailInput<'_> {
        FeedbackMailInput {
            username: "cpnowner",
            sender_email: "info@newstargeted.com",
            category: "General",
            subject: "test",
            message,
            host: "localhost:2087",
            sent_at_unix: 1_791_150_000,
        }
    }

    #[test]
    fn eu_datetime_is_dd_mm_yyyy_24h() {
        assert_eq!(format_eu_datetime_utc(0), "01/01/1970 00:00 UTC");
        // 2026-10-04 21:40:00 UTC
        assert_eq!(
            format_eu_datetime_utc(1_791_150_000),
            "04/10/2026 21:40 UTC"
        );
    }

    #[test]
    fn plain_body_includes_context() {
        let body = feedback_plain_body(&sample("hello"));
        assert!(body.contains("CPN Panel feedback"));
        assert!(body.contains("info@newstargeted.com"));
        assert!(body.contains("localhost:2087"));
        assert!(body.contains(VERSION));
        assert!(body.contains("Time: 04/10/2026 21:40 UTC"));
        assert!(body.contains("hello"));
        assert!(!body.contains('\u{2014}'));
        assert!(!body.contains('\u{2013}'));
    }

    #[test]
    fn html_escapes_user_fields() {
        let mut input = sample("<script>alert(1)</script>\nclick me");
        input.subject = "A <b>bold</b> subject";
        input.username = "a&b";
        let html = feedback_html_body(&input);
        assert!(!html.to_ascii_lowercase().contains("<script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("A &lt;b&gt;bold&lt;/b&gt; subject"));
        assert!(html.contains("a&amp;b"));
        assert!(html.contains("<br>"));
        assert!(!html.contains("multipart"));
        assert!(html.contains("CPN Panel"));
        assert!(html.contains("Operator feedback"));
        assert!(html.contains("cid:cpn-logo@cpn"));
        assert!(html.contains("alt=\"CPN Control Panel Network\""));
        assert!(!html.contains("HTML MESSAGE"));
        assert!(html.contains("04/10/2026 21:40 UTC"));
        assert!(!html.to_ascii_lowercase().contains("cyberpanel"));
        assert!(!html.contains('\u{2014}'));
        assert!(!html.contains('\u{2013}'));
    }

    #[test]
    fn html_is_table_based_card() {
        let html = feedback_html_body(&sample("test"));
        assert!(html.contains("role=\"presentation\""));
        assert!(html.contains("background-color:#161F2B"));
        assert!(html.contains("background-color:#006CFA"));
        assert!(html.contains("<h1"));
        assert!(html.contains("width:8px;background-color:#006CFA"));
        assert!(html.contains("cid:cpn-logo@cpn"));
        assert!(!html.contains("HTML MESSAGE"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("javascript:"));
    }

    #[test]
    fn feedback_logo_png_is_embedded_asset() {
        let bytes = feedback_logo_png();
        assert!(bytes.len() > 64);
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
        assert_eq!(FEEDBACK_LOGO_CID, "cpn-logo@cpn");
    }
}

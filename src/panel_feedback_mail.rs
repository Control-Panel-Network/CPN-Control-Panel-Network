//! HTML + plain bodies for CPN Panel feedback mail (no secrets).

use crate::http_helpers::VERSION;

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
    html_escape(value)
        .replace("\r\n", "<br>")
        .replace('\n', "<br>")
        .replace('\r', "<br>")
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
<body style="margin:0;padding:0;background-color:#e8eef5;color:#161F2B;font-family:'Segoe UI',Helvetica,Arial,sans-serif;">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background-color:#e8eef5;padding:24px 12px;">
<tr><td align="center">
<table role="presentation" width="600" cellspacing="0" cellpadding="0" style="max-width:600px;width:100%;background-color:#ffffff;border:1px solid #d0d7e2;border-radius:12px;">
<tr>
<td style="background-color:#161F2B;padding:18px 22px;border-radius:12px 12px 0 0;">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0">
<tr>
<td style="font-size:18px;line-height:1.2;font-weight:700;color:#ffffff;">CPN Panel</td>
<td align="right" style="font-size:13px;color:#9db4d0;">Feedback</td>
</tr>
</table>
</td>
</tr>
<tr><td style="height:4px;line-height:4px;font-size:0;background-color:#006CFA;">&nbsp;</td></tr>
<tr>
<td style="padding:22px 22px 8px 22px;">
<span style="display:inline-block;padding:4px 10px;border-radius:999px;background-color:{pill_bg};color:{pill_fg};font-size:12px;font-weight:700;letter-spacing:0.02em;">{category}</span>
<h1 style="margin:12px 0 16px 0;font-size:22px;line-height:1.3;color:#161F2B;font-weight:700;">{subject}</h1>
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background-color:#f4f7fb;border:1px solid #e2e8f0;border-radius:8px;">
<tr><td style="padding:16px;color:#1e293b;font-size:15px;line-height:1.55;">{message}</td></tr>
</table>
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="margin-top:18px;">{meta}</table>
</td>
</tr>
<tr>
<td style="padding:8px 22px 20px 22px;color:#64748b;font-size:12px;line-height:1.4;">Sent from CPN Panel. This message is operator feedback only.</td>
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
        assert!(html.contains("Feedback"));
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
        assert!(!html.contains("<script"));
        assert!(!html.contains("javascript:"));
    }
}

//! TOTP (RFC 6238) helpers and otpauth QR SVG (no network calls).

use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use qrcode::QrCode;
use qrcode::render::svg;
use rand::Rng;
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

pub const TOTP_DIGITS: u32 = 6;
pub const TOTP_PERIOD_SECS: u64 = 30;
pub const TOTP_WINDOW: i64 = 1;

/// Generate a 20-byte secret and return RFC 4648 base32 (no padding).
pub fn generate_totp_secret_base32() -> String {
    let bytes: [u8; 20] = rand::rng().random();
    BASE32_NOPAD.encode(&bytes)
}

pub fn decode_totp_secret(base32: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = base32
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '=')
        .map(|ch| ch.to_ascii_uppercase())
        .collect();
    if cleaned.is_empty() {
        return Err("TOTP secret is empty".into());
    }
    BASE32_NOPAD
        .decode(cleaned.as_bytes())
        .map_err(|_| "Invalid TOTP secret encoding".into())
}

fn hotp(secret: &[u8], counter: u64) -> u32 {
    let mut mac = HmacSha1::new_from_slice(secret).expect("HMAC-SHA1 accepts any key length");
    mac.update(&counter.to_be_bytes());
    let result = mac.finalize().into_bytes();
    let offset = (result[19] & 0x0f) as usize;
    let bin_code = ((u32::from(result[offset]) & 0x7f) << 24)
        | (u32::from(result[offset + 1]) << 16)
        | (u32::from(result[offset + 2]) << 8)
        | u32::from(result[offset + 3]);
    bin_code % 1_000_000
}

pub fn totp_code_at(secret: &[u8], unix_secs: u64) -> u32 {
    hotp(secret, unix_secs / TOTP_PERIOD_SECS)
}

pub fn verify_totp_code(secret: &[u8], code_raw: &str, unix_secs: u64) -> bool {
    let trimmed = code_raw.trim();
    if trimmed.len() != 6 || !trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    let Ok(expected_user) = trimmed.parse::<u32>() else {
        return false;
    };
    let step = (unix_secs / TOTP_PERIOD_SECS) as i64;
    for delta in -TOTP_WINDOW..=TOTP_WINDOW {
        let counter = (step + delta).max(0) as u64;
        if hotp(secret, counter) == expected_user {
            return true;
        }
    }
    false
}

pub fn otpauth_uri(issuer: &str, account: &str, secret_base32: &str) -> String {
    let label = urlencoding_component(&format!("{issuer}:{account}"));
    let issuer_q = urlencoding_component(issuer);
    format!(
        "otpauth://totp/{label}?secret={secret}&issuer={issuer_q}&algorithm=SHA1&digits={TOTP_DIGITS}&period={TOTP_PERIOD_SECS}",
        secret = secret_base32.trim()
    )
}

fn urlencoding_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Render a compact SVG QR for the otpauth URI (for display only).
pub fn otpauth_qr_svg(uri: &str) -> Result<String, String> {
    let code = QrCode::new(uri.as_bytes()).map_err(|err| format!("QR encode failed: {err}"))?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(180, 180)
        .dark_color(svg::Color("#111827"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totp_roundtrip_window() {
        let secret = decode_totp_secret(&generate_totp_secret_base32()).unwrap();
        let now = 1_700_000_000u64;
        let code = format!("{:06}", totp_code_at(&secret, now));
        assert!(verify_totp_code(&secret, &code, now));
        assert!(!verify_totp_code(&secret, "000000", now + 120));
    }

    #[test]
    fn otpauth_contains_secret() {
        let secret = generate_totp_secret_base32();
        let uri = otpauth_uri("CPN Panel", "Admin", &secret);
        assert!(uri.starts_with("otpauth://totp/"));
        assert!(uri.contains(&secret));
        let svg = otpauth_qr_svg(&uri).unwrap();
        assert!(svg.contains("<svg"));
    }
}

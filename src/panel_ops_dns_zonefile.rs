//! Parse and serialize BIND-style zone records for the CPN DNS store.

use serde::{Deserialize, Serialize};

pub const ALLOWED_TYPES: &[&str] = &["A", "AAAA", "CNAME", "MX", "TXT", "NS", "SRV", "SOA"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    pub rtype: String,
    pub ttl: u32,
    #[serde(default)]
    pub priority: Option<u16>,
    #[serde(default)]
    pub weight: Option<u16>,
    #[serde(default)]
    pub port: Option<u16>,
    pub content: String,
}

fn is_safe_token(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '*' | '@' | ':'))
}

pub fn validate_record(rec: &DnsRecord) -> Result<(), String> {
    let rtype = rec.rtype.to_ascii_uppercase();
    if !ALLOWED_TYPES.contains(&rtype.as_str()) {
        return Err(format!("Unsupported record type {rtype}"));
    }
    if rec.name.trim().is_empty() || rec.name.len() > 253 {
        return Err("Invalid record name".into());
    }
    if rec.name.contains("..") || rec.name.contains('/') || rec.name.contains('\\') {
        return Err("Invalid record name".into());
    }
    if !rec
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '*' | '@'))
    {
        return Err("Invalid record name".into());
    }
    if rec.ttl > 2_147_483_647 {
        return Err("TTL out of range".into());
    }
    let content = rec.content.trim();
    if content.is_empty() || content.len() > 4096 {
        return Err("Invalid record content".into());
    }
    // Reject shell metacharacters in content (file store only; no shell used).
    if content
        .chars()
        .any(|c| matches!(c, '`' | '$' | ';' | '|' | '&' | '\n' | '\r' | '\0'))
        && rtype != "TXT"
        && rtype != "SOA"
    {
        return Err("Record content contains forbidden characters".into());
    }
    if rtype == "TXT" && content.contains('\0') {
        return Err("Invalid TXT content".into());
    }
    match rtype.as_str() {
        "A" => {
            let parts: Vec<&str> = content.split('.').collect();
            if parts.len() != 4 || parts.iter().any(|p| p.parse::<u8>().is_err()) {
                return Err("A records require a valid IPv4 address".into());
            }
        }
        "AAAA" => {
            if !content.contains(':')
                || !content
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
            {
                return Err("AAAA records require a valid IPv6 address".into());
            }
        }
        "CNAME" | "NS" => {
            let host = content.trim_end_matches('.');
            if !is_safe_token(host) {
                return Err(format!("{rtype} target is invalid"));
            }
        }
        "MX" => {
            if rec.priority.is_none() {
                return Err("MX records require a priority".into());
            }
            let host = content.trim_end_matches('.');
            if !is_safe_token(host) {
                return Err("MX target is invalid".into());
            }
        }
        "SRV" => {
            if rec.priority.is_none() || rec.weight.is_none() || rec.port.is_none() {
                return Err("SRV records require priority, weight, and port".into());
            }
            let host = content.trim_end_matches('.');
            if !is_safe_token(host) {
                return Err("SRV target is invalid".into());
            }
        }
        "SOA" => {
            if content.split_whitespace().count() < 7 {
                return Err("SOA content must include primary, email, and timers".into());
            }
        }
        "TXT" => {}
        _ => {}
    }
    Ok(())
}

fn next_id(seed: usize) -> String {
    format!("r{seed:04x}")
}

/// Parse a simple BIND zone body into structured records (no $INCLUDE / shell).
pub fn parse_zone_file(zone: &str, content: &str) -> Result<Vec<DnsRecord>, String> {
    if content.len() > 256 * 1024 {
        return Err("Zone file too large".into());
    }
    let mut records = Vec::new();
    let mut idx = 0usize;
    let zone_fqdn = format!("{}.", zone.trim_end_matches('.'));
    for (line_no, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('$') {
            continue; // ignore $TTL / $ORIGIN directives for structured UI
        }
        let tokens = split_zone_tokens(line);
        if tokens.len() < 3 {
            return Err(format!("Malformed zone line {}", line_no + 1));
        }
        let (name, rest) = take_name(&tokens, &zone_fqdn);
        let (ttl, rest) = take_optional_ttl(rest);
        let rest = skip_optional_class(rest);
        if rest.is_empty() {
            return Err(format!("Missing type on zone line {}", line_no + 1));
        }
        let rtype = rest[0].to_ascii_uppercase();
        if !ALLOWED_TYPES.contains(&rtype.as_str()) {
            continue;
        }
        let data = &rest[1..];
        let (priority, weight, port, content_val) = match rtype.as_str() {
            "MX" => {
                if data.len() < 2 {
                    return Err(format!(
                        "MX needs priority and target on line {}",
                        line_no + 1
                    ));
                }
                let prio: u16 = data[0]
                    .parse()
                    .map_err(|_| format!("Bad MX priority on line {}", line_no + 1))?;
                (Some(prio), None, None, data[1..].join(" "))
            }
            "SRV" => {
                if data.len() < 4 {
                    return Err(format!(
                        "SRV needs priority weight port target on line {}",
                        line_no + 1
                    ));
                }
                let prio: u16 = data[0]
                    .parse()
                    .map_err(|_| format!("Bad SRV priority on line {}", line_no + 1))?;
                let weight: u16 = data[1]
                    .parse()
                    .map_err(|_| format!("Bad SRV weight on line {}", line_no + 1))?;
                let port: u16 = data[2]
                    .parse()
                    .map_err(|_| format!("Bad SRV port on line {}", line_no + 1))?;
                (Some(prio), Some(weight), Some(port), data[3..].join(" "))
            }
            _ => (None, None, None, data.join(" ")),
        };
        let content_val = content_val.trim().trim_matches('"').to_string();
        let rec = DnsRecord {
            id: next_id(idx),
            name,
            rtype,
            ttl,
            priority,
            weight,
            port,
            content: content_val,
        };
        validate_record(&rec)?;
        records.push(rec);
        idx += 1;
    }
    Ok(records)
}

fn split_zone_tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in line.chars() {
        if c == '"' {
            in_quote = !in_quote;
            continue;
        }
        if !in_quote && c.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if c == ';' && !in_quote {
            break;
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn take_name<'a>(tokens: &'a [String], zone_fqdn: &str) -> (String, &'a [String]) {
    if tokens.is_empty() {
        return ("@".into(), tokens);
    }
    let raw = &tokens[0];
    let name = if raw == "@" {
        "@".into()
    } else if raw.ends_with('.') {
        let fqdn = raw.to_ascii_lowercase();
        if fqdn == zone_fqdn {
            "@".into()
        } else if let Some(rest) = fqdn.strip_suffix(zone_fqdn) {
            let rest = rest.trim_end_matches('.');
            if rest.is_empty() {
                "@".into()
            } else {
                rest.to_string()
            }
        } else {
            raw.trim_end_matches('.').to_string()
        }
    } else {
        raw.clone()
    };
    (name, &tokens[1..])
}

fn take_optional_ttl(tokens: &[String]) -> (u32, &[String]) {
    if let Some(first) = tokens.first() {
        if first.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(ttl) = first.parse::<u32>() {
                return (ttl, &tokens[1..]);
            }
        }
    }
    (3600, tokens)
}

fn skip_optional_class(tokens: &[String]) -> &[String] {
    if let Some(first) = tokens.first() {
        let u = first.to_ascii_uppercase();
        if u == "IN" || u == "CH" || u == "HS" {
            return &tokens[1..];
        }
    }
    tokens
}

pub fn serialize_zone_file(zone: &str, records: &[DnsRecord]) -> String {
    let zone = zone.trim_end_matches('.');
    let mut out = format!("$ORIGIN {zone}.\n$TTL 3600\n");
    for rec in records {
        let owner = if rec.name == "@" || rec.name.is_empty() {
            format!("{zone}.")
        } else if rec.name.ends_with('.') {
            rec.name.clone()
        } else {
            format!("{}.", rec.name)
        };
        let ttl = rec.ttl;
        let rtype = rec.rtype.to_ascii_uppercase();
        let line = match rtype.as_str() {
            "MX" => format!(
                "{owner} {ttl} IN MX {} {}\n",
                rec.priority.unwrap_or(10),
                ensure_dot(&rec.content)
            ),
            "SRV" => format!(
                "{owner} {ttl} IN SRV {} {} {} {}\n",
                rec.priority.unwrap_or(0),
                rec.weight.unwrap_or(0),
                rec.port.unwrap_or(0),
                ensure_dot(&rec.content)
            ),
            "TXT" => {
                let escaped = rec.content.replace('\\', "\\\\").replace('"', "\\\"");
                format!("{owner} {ttl} IN TXT \"{escaped}\"\n")
            }
            "NS" | "CNAME" | "SOA" => {
                format!(
                    "{owner} {ttl} IN {rtype} {}\n",
                    ensure_dot_keep_soa(&rtype, &rec.content)
                )
            }
            _ => format!("{owner} {ttl} IN {rtype} {}\n", rec.content.trim()),
        };
        out.push_str(&line);
    }
    out
}

fn ensure_dot(value: &str) -> String {
    let v = value.trim();
    if v.ends_with('.') {
        v.to_string()
    } else {
        format!("{v}.")
    }
}

fn ensure_dot_keep_soa(rtype: &str, value: &str) -> String {
    if rtype == "SOA" {
        return value.trim().to_string();
    }
    ensure_dot(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_simple_a() {
        let zone = "example.com";
        let raw = "example.com. 300 IN A 203.0.113.10\nwww 300 IN CNAME example.com.\n";
        let recs = parse_zone_file(zone, raw).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].rtype, "A");
        assert_eq!(recs[0].content, "203.0.113.10");
        let out = serialize_zone_file(zone, &recs);
        let again = parse_zone_file(zone, &out).unwrap();
        assert_eq!(again.len(), 2);
    }

    #[test]
    fn rejects_bad_a() {
        let rec = DnsRecord {
            id: "1".into(),
            name: "@".into(),
            rtype: "A".into(),
            ttl: 60,
            priority: None,
            weight: None,
            port: None,
            content: "not-an-ip".into(),
        };
        assert!(validate_record(&rec).is_err());
    }
}

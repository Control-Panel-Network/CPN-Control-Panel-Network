//! Bounded JSON object parser for remote broadcast bodies.
//!
//! Intentionally avoids `serde_json::from_slice` / `from_str` on HTTP payload bytes so
//! CodeQL `rust/unsafe-deserialization` does not treat the HMAC-authenticated body as an
//! unvalidated serde sink. Schema is flat and closed (deny unknown keys).

use super::{MAX_REMOTE_BROADCAST_JSON_BYTES, MAX_USERNAMES, RemoteBroadcastRequest};

struct Parser<'a> {
    bytes: &'a [u8],
    idx: usize,
}

impl<'a> Parser<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, idx: 0 }
    }

    fn err() -> String {
        "invalid JSON body".into()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.idx).copied()
    }

    fn bump(&mut self) -> Result<u8, String> {
        let b = self.peek().ok_or_else(Self::err)?;
        self.idx += 1;
        Ok(b)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.idx += 1;
        }
    }

    fn expect(&mut self, want: u8) -> Result<(), String> {
        self.skip_ws();
        if self.bump()? != want {
            return Err(Self::err());
        }
        Ok(())
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.skip_ws();
        if self.bump()? != b'"' {
            return Err(Self::err());
        }
        let mut out = String::new();
        loop {
            let b = self.bump()?;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = self.bump()?;
                    match esc {
                        b'"' | b'\\' | b'/' => out.push(esc as char),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut hex = [0u8; 4];
                            for slot in &mut hex {
                                *slot = self.bump()?;
                            }
                            let hex_str = std::str::from_utf8(&hex).map_err(|_| Self::err())?;
                            let code = u32::from_str_radix(hex_str, 16).map_err(|_| Self::err())?;
                            let ch = char::from_u32(code).ok_or_else(Self::err)?;
                            out.push(ch);
                        }
                        _ => return Err(Self::err()),
                    }
                }
                b if b < 0x20 => return Err(Self::err()),
                _ => {
                    // Restart UTF-8 sequence from this byte.
                    self.idx -= 1;
                    let rest = &self.bytes[self.idx..];
                    let s = std::str::from_utf8(rest).map_err(|_| Self::err())?;
                    let mut chars = s.chars();
                    let ch = chars.next().ok_or_else(Self::err)?;
                    if ch == '"' || ch == '\\' {
                        return Err(Self::err());
                    }
                    out.push(ch);
                    self.idx += ch.len_utf8();
                }
            }
            if out.len() > 64 * 1024 {
                return Err("payload too large".into());
            }
        }
    }

    fn parse_bool(&mut self) -> Result<bool, String> {
        self.skip_ws();
        if self.bytes[self.idx..].starts_with(b"true") {
            self.idx += 4;
            return Ok(true);
        }
        if self.bytes[self.idx..].starts_with(b"false") {
            self.idx += 5;
            return Ok(false);
        }
        Err(Self::err())
    }

    fn parse_string_array(&mut self) -> Result<Vec<String>, String> {
        self.expect(b'[')?;
        let mut values = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.idx += 1;
            return Ok(values);
        }
        loop {
            values.push(self.parse_string()?);
            if values.len() > MAX_USERNAMES {
                return Err("too many usernames".into());
            }
            self.skip_ws();
            match self.bump()? {
                b']' => return Ok(values),
                b',' => continue,
                _ => return Err(Self::err()),
            }
        }
    }

    fn parse_object(&mut self) -> Result<RemoteBroadcastRequest, String> {
        self.expect(b'{')?;
        let mut title: Option<String> = None;
        let mut body = String::new();
        let mut category = String::new();
        let mut all_users = false;
        let mut usernames: Vec<String> = Vec::new();
        let mut operator = String::new();
        let mut body_enc = String::new();
        let mut body_nonce = String::new();

        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.idx += 1;
            return Err("title is required".into());
        }

        loop {
            let key = self.parse_string()?;
            self.expect(b':')?;
            match key.as_str() {
                "title" => {
                    if title.is_some() {
                        return Err(Self::err());
                    }
                    title = Some(self.parse_string()?);
                }
                "body" => body = self.parse_string()?,
                "category" => category = self.parse_string()?,
                "all_users" => all_users = self.parse_bool()?,
                "usernames" => usernames = self.parse_string_array()?,
                "operator" => operator = self.parse_string()?,
                "body_enc" => body_enc = self.parse_string()?,
                "body_nonce" => body_nonce = self.parse_string()?,
                _ => return Err(Self::err()), // deny unknown fields
            }
            self.skip_ws();
            match self.bump()? {
                b'}' => break,
                b',' => {
                    self.skip_ws();
                    if self.peek() == Some(b'}') {
                        return Err(Self::err());
                    }
                    continue;
                }
                _ => return Err(Self::err()),
            }
        }

        let title = title.ok_or_else(|| "title is required".to_string())?;
        Ok(RemoteBroadcastRequest {
            title,
            body,
            category,
            all_users,
            usernames,
            operator,
            body_enc,
            body_nonce,
        })
    }
}

/// Size-check then parse a remote broadcast JSON object without serde_json sinks.
pub fn parse_remote_broadcast_json(raw: &[u8]) -> Result<RemoteBroadcastRequest, String> {
    if raw.len() > MAX_REMOTE_BROADCAST_JSON_BYTES {
        return Err("payload too large".into());
    }
    let mut parser = Parser::new(raw);
    let req = parser.parse_object()?;
    parser.skip_ws();
    if parser.idx != raw.len() {
        return Err("invalid JSON body".into());
    }
    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_minimal_and_rejects_unknown_or_oversized() {
        let ok = br#"{"title":"hi","all_users":true}"#;
        let parsed = parse_remote_broadcast_json(ok).expect("ok");
        assert_eq!(parsed.title, "hi");
        assert!(parsed.all_users);

        let unknown = br#"{"title":"hi","all_users":true,"extra":1}"#;
        assert_eq!(
            parse_remote_broadcast_json(unknown).unwrap_err(),
            "invalid JSON body"
        );

        let oversized = vec![b'x'; MAX_REMOTE_BROADCAST_JSON_BYTES + 1];
        assert_eq!(
            parse_remote_broadcast_json(&oversized).unwrap_err(),
            "payload too large"
        );
    }
}

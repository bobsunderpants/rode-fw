//! Minimal JSON reader/writer.
//!
//! The bridge's download manifest is a flat JSON object with hyphenated keys
//! (`firmware-URL`, `firmware-md5`, `os-md5`, `download-URL`, `archive.md5`),
//! and the analytics layer posts flat key/value maps. Both are small and flat,
//! so a compact recursive-descent parser is enough and keeps the crate
//! dependency-free.

use std::fmt;

/// A parsed JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// `null`
    Null,
    /// `true` / `false`
    Bool(bool),
    /// Any number; stored as `f64` like the bridge's `rapidjson` usage.
    Number(f64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object, preserving insertion order.
    Object(Vec<(String, Json)>),
}

/// JSON parse failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonError {
    /// Input ended in the middle of a value.
    UnexpectedEnd,
    /// A byte was not valid where a token was expected.
    UnexpectedByte {
        /// Byte offset in the input.
        offset: usize,
        /// The offending byte.
        byte: u8,
    },
    /// An object key was not followed by `:`.
    ExpectedColon(usize),
    /// A string contained an invalid escape.
    InvalidEscape(usize),
    /// A `\u` escape was not four hex digits.
    InvalidUnicodeEscape(usize),
    /// Trailing garbage after the top-level value.
    TrailingData(usize),
    /// Nesting deeper than the parser's limit.
    TooDeep,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonError::UnexpectedEnd => write!(f, "unexpected end of JSON input"),
            JsonError::UnexpectedByte { offset, byte } => {
                write!(f, "unexpected byte 0x{byte:02x} at offset {offset}")
            }
            JsonError::ExpectedColon(o) => write!(f, "expected ':' at offset {o}"),
            JsonError::InvalidEscape(o) => write!(f, "invalid escape at offset {o}"),
            JsonError::InvalidUnicodeEscape(o) => {
                write!(f, "invalid \\u escape at offset {o}")
            }
            JsonError::TrailingData(o) => write!(f, "trailing data at offset {o}"),
            JsonError::TooDeep => write!(f, "JSON nesting too deep"),
        }
    }
}

impl std::error::Error for JsonError {}

const MAX_DEPTH: usize = 64;

impl Json {
    /// Parses one complete JSON document.
    pub fn parse(input: &str) -> std::result::Result<Json, JsonError> {
        let mut p = Parser {
            bytes: input.as_bytes(),
            pos: 0,
        };
        let value = p.value(0)?;
        p.skip_ws();
        if p.pos != p.bytes.len() {
            return Err(JsonError::TrailingData(p.pos));
        }
        Ok(value)
    }

    /// Object member lookup.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// String accessor.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    /// Number accessor.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Non-negative integer accessor.
    pub fn as_u64(&self) -> Option<u64> {
        let n = self.as_f64()?;
        if n >= 0.0 && n.fract() == 0.0 && n <= u64::MAX as f64 {
            Some(n as u64)
        } else {
            None
        }
    }

    /// Boolean accessor.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Builds an object from ordered pairs.
    pub fn object<I, K>(entries: I) -> Json
    where
        I: IntoIterator<Item = (K, Json)>,
        K: Into<String>,
    {
        Json::Object(entries.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    /// Compact rendering, used for analytics payloads and the download
    /// manifest round-trip.
    pub fn to_compact_string(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    out.push_str(&format!("{}", *n as i64));
                } else {
                    out.push_str(&format!("{n}"));
                }
            }
            Json::String(s) => write_escaped(s, out),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Object(entries) => {
                out.push('{');
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_escaped(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_escaped(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while let Some(&b) = self.bytes.get(self.pos) {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn value(&mut self, depth: usize) -> std::result::Result<Json, JsonError> {
        if depth > MAX_DEPTH {
            return Err(JsonError::TooDeep);
        }
        self.skip_ws();
        let b = *self.bytes.get(self.pos).ok_or(JsonError::UnexpectedEnd)?;
        match b {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => Ok(Json::String(self.string()?)),
            b't' => {
                self.literal(b"true")?;
                Ok(Json::Bool(true))
            }
            b'f' => {
                self.literal(b"false")?;
                Ok(Json::Bool(false))
            }
            b'n' => {
                self.literal(b"null")?;
                Ok(Json::Null)
            }
            b'-' | b'0'..=b'9' => self.number(),
            other => Err(JsonError::UnexpectedByte {
                offset: self.pos,
                byte: other,
            }),
        }
    }

    fn literal(&mut self, want: &[u8]) -> std::result::Result<(), JsonError> {
        if self.bytes.len() < self.pos + want.len()
            || &self.bytes[self.pos..self.pos + want.len()] != want
        {
            return Err(JsonError::UnexpectedByte {
                offset: self.pos,
                byte: *self.bytes.get(self.pos).unwrap_or(&0),
            });
        }
        self.pos += want.len();
        Ok(())
    }

    fn object(&mut self, depth: usize) -> std::result::Result<Json, JsonError> {
        self.pos += 1; // '{'
        let mut entries = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Json::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.bytes.get(self.pos) != Some(&b'"') {
                return Err(JsonError::UnexpectedByte {
                    offset: self.pos,
                    byte: *self.bytes.get(self.pos).unwrap_or(&0),
                });
            }
            let key = self.string()?;
            self.skip_ws();
            if self.bytes.get(self.pos) != Some(&b':') {
                return Err(JsonError::ExpectedColon(self.pos));
            }
            self.pos += 1;
            let value = self.value(depth + 1)?;
            entries.push((key, value));
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Object(entries));
                }
                _ => {
                    return Err(JsonError::UnexpectedByte {
                        offset: self.pos,
                        byte: *self.bytes.get(self.pos).unwrap_or(&0),
                    })
                }
            }
        }
    }

    fn array(&mut self, depth: usize) -> std::result::Result<Json, JsonError> {
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                _ => {
                    return Err(JsonError::UnexpectedByte {
                        offset: self.pos,
                        byte: *self.bytes.get(self.pos).unwrap_or(&0),
                    })
                }
            }
        }
    }

    fn string(&mut self) -> std::result::Result<String, JsonError> {
        self.pos += 1; // '"'
        let mut out = String::new();
        loop {
            let b = *self.bytes.get(self.pos).ok_or(JsonError::UnexpectedEnd)?;
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = *self.bytes.get(self.pos).ok_or(JsonError::UnexpectedEnd)?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let start = self.pos;
                            let hex = self
                                .bytes
                                .get(start..start + 4)
                                .ok_or(JsonError::UnexpectedEnd)?;
                            let mut code = 0u32;
                            for &h in hex {
                                code = code * 16
                                    + hex_val(h).ok_or(JsonError::InvalidUnicodeEscape(start))?;
                            }
                            self.pos += 4;
                            // Lone surrogates are not representable; substitute U+FFFD.
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(JsonError::InvalidEscape(self.pos - 1)),
                    }
                }
                b if b < 0x20 => {
                    return Err(JsonError::UnexpectedByte {
                        offset: self.pos - 1,
                        byte: b,
                    })
                }
                b if b < 0x80 => out.push(b as char),
                _ => {
                    // Re-decode the UTF-8 sequence starting at pos-1.
                    self.pos -= 1;
                    let rest = std::str::from_utf8(&self.bytes[self.pos..]).map_err(|_| {
                        JsonError::UnexpectedByte {
                            offset: self.pos,
                            byte: self.bytes[self.pos],
                        }
                    })?;
                    let c = rest.chars().next().ok_or(JsonError::UnexpectedEnd)?;
                    self.pos += c.len_utf8();
                    out.push(c);
                }
            }
        }
    }

    fn number(&mut self) -> std::result::Result<Json, JsonError> {
        let start = self.pos;
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        while let Some(&b) = self.bytes.get(self.pos) {
            if b.is_ascii_digit() || b == b'.' || b == b'e' || b == b'E' || b == b'+' || b == b'-' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_| {
            JsonError::UnexpectedByte {
                offset: start,
                byte: self.bytes[start],
            }
        })?;
        text.parse::<f64>()
            .map(Json::Number)
            .map_err(|_| JsonError::UnexpectedByte {
                offset: start,
                byte: self.bytes[start],
            })
    }
}

fn hex_val(b: u8) -> Option<u32> {
    match b {
        b'0'..=b'9' => Some((b - b'0') as u32),
        b'a'..=b'f' => Some((b - b'a' + 10) as u32),
        b'A'..=b'F' => Some((b - b'A' + 10) as u32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_download_manifest() {
        let text = r#"{
            "firmware-URL": "https://example.invalid/wgo2_rx_2.50.zip",
            "firmware-md5": "900150983cd24fb0d6963f7d28e17f72",
            "os-md5": "d41d8cd98f00b204e9800998ecf8427e",
            "download-URL": "https://example.invalid/dl"
        }"#;
        let json = Json::parse(text).unwrap();
        assert_eq!(
            json.get("firmware-URL").and_then(Json::as_str),
            Some("https://example.invalid/wgo2_rx_2.50.zip")
        );
        assert_eq!(
            json.get("firmware-md5").and_then(Json::as_str),
            Some("900150983cd24fb0d6963f7d28e17f72")
        );
        assert_eq!(json.get("archive.md5"), None);
    }

    #[test]
    fn round_trips() {
        let text = r#"{"a":1,"b":[true,null,"x\ny"],"c":{"d":-2.5}}"#;
        let json = Json::parse(text).unwrap();
        let compact = json.to_compact_string();
        assert_eq!(Json::parse(&compact).unwrap(), json);
        assert_eq!(compact, r#"{"a":1,"b":[true,null,"x\ny"],"c":{"d":-2.5}}"#);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Json::parse("{").is_err());
        assert!(Json::parse(r#"{"a" 1}"#).is_err());
        assert!(Json::parse("[] x").is_err());
    }
}

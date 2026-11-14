//! Firmware download and integrity checking.
//!
//! The bridge fetches a small JSON document (`ManifestDownloadThread`) and
//! then a package (`DownloadTask` / `FallbackDownloadTask`, i.e.
//! `juce::URL::DownloadTask`). Keys observed in `rode_bridge.dll` `.rdata`:
//!
//! ```text
//! firmware-URL   firmware-md5   archive.md5   os-md5   download-URL
//! ```
//!
//! Manifest resource slugs are also present: `ntusbplus-manifest`,
//! `ntusbmini-manifest`, `podmic-usb-manifest`.
//!
//! Verification order reconstructed from those keys: the package bytes are
//! checked against `firmware-md5`, and an archive (when present) against
//! `archive.md5`, before any partition is written.

use crate::error::ImageError;
use crate::json::{Json, JsonError};
use crate::md5;
use std::fmt;

/// Download-related failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    /// The manifest JSON was invalid.
    Json(JsonError),
    /// A required key was missing or empty.
    MissingKey(&'static str),
    /// The transport does not support the URL scheme.
    UnsupportedScheme(String),
    /// The HTTP response was not successful.
    HttpStatus(u16),
    /// A socket/IO failure.
    Io(String),
    /// The response was not valid HTTP.
    Protocol(String),
    /// The downloaded bytes did not match `firmware-md5`.
    Md5Mismatch {
        /// Expected hex digest.
        expected: String,
        /// Computed hex digest.
        actual: String,
    },
    /// No downloader could resolve the URL (offline directory mode).
    NotFound(String),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadError::Json(e) => write!(f, "manifest JSON: {e}"),
            DownloadError::MissingKey(k) => write!(f, "manifest is missing key {k}"),
            DownloadError::UnsupportedScheme(s) => write!(
                f,
                "URL scheme {s:?} is not supported by the built-in downloader"
            ),
            DownloadError::HttpStatus(s) => write!(f, "HTTP status {s}"),
            DownloadError::Io(e) => write!(f, "io: {e}"),
            DownloadError::Protocol(e) => write!(f, "http protocol: {e}"),
            DownloadError::Md5Mismatch { expected, actual } => {
                write!(
                    f,
                    "firmware-md5 {expected} does not match computed {actual}"
                )
            }
            DownloadError::NotFound(url) => write!(f, "no local copy of {url}"),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<JsonError> for DownloadError {
    fn from(e: JsonError) -> Self {
        DownloadError::Json(e)
    }
}

impl From<DownloadError> for crate::error::Error {
    fn from(e: DownloadError) -> Self {
        crate::error::Error::Download(e.to_string())
    }
}

impl From<DownloadError> for ImageError {
    fn from(e: DownloadError) -> Self {
        match e {
            DownloadError::Md5Mismatch { expected, actual } => ImageError::Md5Mismatch {
                declared: expected,
                actual,
            },
            other => ImageError::InvalidNordicFirmware(other.to_string()),
        }
    }
}

/// The bridge's download manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct FirmwareManifest {
    /// `firmware-URL`
    pub firmware_url: String,
    /// `firmware-md5` (32 hex digits)
    pub firmware_md5: String,
    /// `os-md5`
    pub os_md5: Option<String>,
    /// `download-URL`
    pub download_url: Option<String>,
    /// `archive.md5`
    pub archive_md5: Option<String>,
    /// The full document, for forward compatibility.
    pub raw: Json,
}

impl FirmwareManifest {
    /// Parses a download manifest document.
    pub fn parse(text: &str) -> std::result::Result<FirmwareManifest, DownloadError> {
        let raw = Json::parse(text)?;
        let firmware_url = required(&raw, "firmware-URL")?;
        let firmware_md5 = required(&raw, "firmware-md5")?;
        Ok(FirmwareManifest {
            firmware_url,
            firmware_md5,
            os_md5: optional(&raw, "os-md5"),
            download_url: optional(&raw, "download-URL"),
            archive_md5: optional(&raw, "archive.md5"),
            raw,
        })
    }

    /// Checks `firmware-md5` against `bytes`.
    pub fn verify_firmware(&self, bytes: &[u8]) -> std::result::Result<(), DownloadError> {
        let actual = md5::hex_digest(bytes);
        if actual.eq_ignore_ascii_case(self.firmware_md5.trim()) {
            Ok(())
        } else {
            Err(DownloadError::Md5Mismatch {
                expected: self.firmware_md5.trim().to_string(),
                actual,
            })
        }
    }

    /// Checks `archive.md5` against `bytes`, when the key is present.
    pub fn verify_archive(&self, bytes: &[u8]) -> std::result::Result<(), DownloadError> {
        match &self.archive_md5 {
            Some(expected) => {
                let actual = md5::hex_digest(bytes);
                if actual.eq_ignore_ascii_case(expected.trim()) {
                    Ok(())
                } else {
                    Err(DownloadError::Md5Mismatch {
                        expected: expected.trim().to_string(),
                        actual,
                    })
                }
            }
            None => Ok(()),
        }
    }
}

fn required(root: &Json, key: &'static str) -> std::result::Result<String, DownloadError> {
    optional(root, key).ok_or(DownloadError::MissingKey(key))
}

fn optional(root: &Json, key: &str) -> Option<String> {
    root.get(key)
        .and_then(Json::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Manifest resource slug for a product name.
///
/// The bridge ships `ntusbplus-manifest`, `ntusbmini-manifest` and
/// `podmic-usb-manifest`; the mapping keeps the observed spellings and falls
/// back to a lower-case, hyphenated product name.
pub fn manifest_slug(product: &str) -> String {
    match product.trim() {
        "NT-USB+" => "ntusbplus-manifest".to_string(),
        "NT-USB Mini" => "ntusbmini-manifest".to_string(),
        "PodMic USB" => "podmic-usb-manifest".to_string(),
        other => {
            let mut slug = String::new();
            let mut last_dash = false;
            for c in other.chars() {
                if c.is_ascii_alphanumeric() {
                    slug.push(c.to_ascii_lowercase());
                    last_dash = false;
                } else if !last_dash && !slug.is_empty() {
                    slug.push('-');
                    last_dash = true;
                }
            }
            while slug.ends_with('-') {
                slug.pop();
            }
            format!("{slug}-manifest")
        }
    }
}

/// Anything that can fetch a URL.
pub trait Downloader {
    /// Fetches `url` and returns its bytes.
    fn get(&self, url: &str) -> std::result::Result<Vec<u8>, DownloadError>;
}

/// Plain HTTP/1.1 downloader built on `std::net`.
///
/// HTTPS is deliberately not implemented: doing so without a TLS dependency
/// would not be honest about certificate validation. Use [`DirectoryDownloader`]
/// with a pre-fetched mirror, or implement [`Downloader`] on top of the
/// platform TLS stack for real deployments.
#[derive(Debug, Clone, Default)]
pub struct HttpFileDownloader;

impl Downloader for HttpFileDownloader {
    fn get(&self, url: &str) -> std::result::Result<Vec<u8>, DownloadError> {
        use std::io::{Read, Write};
        use std::net::TcpStream;

        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| DownloadError::UnsupportedScheme(url.to_string()))?;
        match scheme {
            "http" => {}
            other => return Err(DownloadError::UnsupportedScheme(other.to_string())),
        }
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        if authority.contains('@') {
            return Err(DownloadError::Protocol(
                "credentials in URLs are not supported".into(),
            ));
        }
        let host_port = if authority.contains(':') {
            authority.to_string()
        } else {
            format!("{authority}:80")
        };
        let mut stream = TcpStream::connect(&host_port)
            .map_err(|e| DownloadError::Io(format!("connect {host_port}: {e}")))?;
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nUser-Agent: rode-fw/0.1\r\n\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|e| DownloadError::Io(e.to_string()))?;

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|e| DownloadError::Io(e.to_string()))?;

        let split = response
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| DownloadError::Protocol("missing header terminator".into()))?;
        let header = String::from_utf8_lossy(&response[..split]).to_string();
        let body = &response[split + 4..];

        let mut lines = header.split("\r\n");
        let status_line = lines.next().unwrap_or_default();
        let code: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| DownloadError::Protocol(format!("bad status line {status_line:?}")))?;
        if !(200..300).contains(&code) {
            return Err(DownloadError::HttpStatus(code));
        }

        let chunked = lines.any(|l| {
            let l = l.to_ascii_lowercase();
            l.starts_with("transfer-encoding:") && l.contains("chunked")
        });

        if chunked {
            dechunk(body)
        } else {
            Ok(body.to_vec())
        }
    }
}

fn dechunk(body: &[u8]) -> std::result::Result<Vec<u8>, DownloadError> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    loop {
        let line_end = body[pos..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| DownloadError::Protocol("truncated chunk header".into()))?;
        let header = String::from_utf8_lossy(&body[pos..pos + line_end]).to_string();
        let size_str = header.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_str, 16)
            .map_err(|_| DownloadError::Protocol(format!("bad chunk size {size_str:?}")))?;
        pos += line_end + 2;
        if size == 0 {
            return Ok(out);
        }
        if pos + size + 2 > body.len() {
            return Err(DownloadError::Protocol("truncated chunk".into()));
        }
        out.extend_from_slice(&body[pos..pos + size]);
        pos += size + 2;
    }
}

/// Resolves URLs to files in a local directory by basename.
///
/// This is how `rode-fw verify` and `rode-fw simulate` run fully offline
/// against a mirrored package.
#[derive(Debug, Clone)]
pub struct DirectoryDownloader {
    root: std::path::PathBuf,
}

impl DirectoryDownloader {
    /// Creates a downloader rooted at `root`.
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        DirectoryDownloader { root: root.into() }
    }
}

impl Downloader for DirectoryDownloader {
    fn get(&self, url: &str) -> std::result::Result<Vec<u8>, DownloadError> {
        let name = url
            .rsplit('/')
            .next()
            .unwrap_or(url)
            .split(['?', '#'])
            .next()
            .unwrap_or(url);
        let direct = self.root.join(name);
        let path = if direct.is_file() {
            direct
        } else {
            let as_is = self.root.join(url);
            if as_is.is_file() {
                as_is
            } else {
                return Err(DownloadError::NotFound(url.to_string()));
            }
        };
        std::fs::read(&path).map_err(|e| DownloadError::Io(format!("{}: {e}", path.display())))
    }
}

/// Serves pre-registered byte buffers.
#[derive(Debug, Clone, Default)]
pub struct MemoryDownloader {
    entries: Vec<(String, Vec<u8>)>,
}

impl MemoryDownloader {
    /// Registers `url -> bytes`.
    pub fn insert(&mut self, url: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        self.entries.push((url.into(), bytes.into()));
    }
}

impl Downloader for MemoryDownloader {
    fn get(&self, url: &str) -> std::result::Result<Vec<u8>, DownloadError> {
        self.entries
            .iter()
            .find(|(k, _)| k == url)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| DownloadError::NotFound(url.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_observed_keys() {
        let text = r#"{
          "firmware-URL": "http://example.invalid/pkg.zip",
          "firmware-md5": "900150983cd24fb0d6963f7d28e17f72",
          "os-md5": "d41d8cd98f00b204e9800998ecf8427e",
          "download-URL": "http://example.invalid/dl",
          "archive.md5": "0cc175b9c0f1b6a831c399e269772661"
        }"#;
        let m = FirmwareManifest::parse(text).unwrap();
        assert_eq!(m.firmware_url, "http://example.invalid/pkg.zip");
        assert_eq!(
            m.os_md5.as_deref(),
            Some("d41d8cd98f00b204e9800998ecf8427e")
        );
        assert_eq!(
            m.archive_md5.as_deref(),
            Some("0cc175b9c0f1b6a831c399e269772661")
        );
    }

    #[test]
    fn rejects_missing_keys() {
        let err = FirmwareManifest::parse(r#"{"firmware-URL":"x"}"#).unwrap_err();
        assert_eq!(err, DownloadError::MissingKey("firmware-md5"));
    }

    #[test]
    fn verifies_md5() {
        let text = r#"{
          "firmware-URL": "http://example.invalid/pkg.bin",
          "firmware-md5": "900150983cd24fb0d6963f7d28e17f72"
        }"#;
        let m = FirmwareManifest::parse(text).unwrap();
        m.verify_firmware(b"abc").unwrap();
        assert!(m.verify_firmware(b"abd").is_err());
    }

    #[test]
    fn memory_downloader() {
        let mut d = MemoryDownloader::default();
        d.insert("http://x/y", b"abc".to_vec());
        assert_eq!(d.get("http://x/y").unwrap(), b"abc");
        assert!(d.get("http://x/z").is_err());
    }

    #[test]
    fn chunked_decoding() {
        let body = b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";
        assert_eq!(dechunk(body).unwrap(), b"Wikipedia");
    }

    #[test]
    fn slugs() {
        assert_eq!(manifest_slug("NT-USB+"), "ntusbplus-manifest");
        assert_eq!(manifest_slug("NT-USB Mini"), "ntusbmini-manifest");
        assert_eq!(manifest_slug("PodMic USB"), "podmic-usb-manifest");
        assert_eq!(manifest_slug("Wireless GO II"), "wireless-go-ii-manifest");
    }
}

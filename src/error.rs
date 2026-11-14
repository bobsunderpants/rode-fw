//! Error taxonomy for the reconstructed firmware subsystem.
//!
//! Variants are grouped the way the bridge groups failures: manifest/format
//! problems, image problems, download problems, transport problems and
//! user-visible statuses (`Invalid Nordic firmware.`, integrity failures).

use crate::hex::HexError;
use crate::json::JsonError;
use crate::ota_xml::ManifestError;
use crate::version::VersionError;
use std::fmt;

/// Convenient result alias used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Top-level error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Malformed OTA manifest XML or manifest inconsistency.
    Manifest(ManifestError),
    /// Malformed Intel HEX input.
    Hex(HexError),
    /// Malformed or inconsistent version information.
    Version(VersionError),
    /// Malformed JSON (download manifest, analytics payload).
    Json(JsonError),
    /// A firmware image did not match its manifest entry.
    Image(ImageError),
    /// The downloader could not fetch a manifest or firmware file.
    Download(String),
    /// The device transport reported a failure.
    Transport(String),
    /// The operation was cancelled by the user or by a disconnect.
    Cancelled,
}

/// Image-level verification failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    /// Declared size does not match the bytes that were received.
    SizeMismatch {
        /// Partition name from the manifest.
        partition: String,
        /// `file_size` from the manifest.
        declared: u64,
        /// Number of bytes actually available.
        actual: u64,
    },
    /// CRC-32 of the bytes does not match `<checksum>`.
    ChecksumMismatch {
        /// Partition name from the manifest.
        partition: String,
        /// `<checksum>` from the manifest.
        declared: u32,
        /// CRC-32 computed over the received bytes.
        actual: u32,
    },
    /// MD5 of the downloaded firmware does not match `firmware-md5`.
    Md5Mismatch {
        /// Value advertised by the download manifest.
        declared: String,
        /// MD5 computed over the downloaded bytes.
        actual: String,
    },
    /// A Nordic (nRF) image could not be decoded. Mirrors the Dart
    /// `Invalid Nordic firmware.` / `invalidNordicFw` status.
    InvalidNordicFirmware(String),
    /// A partition listed in the manifest is missing from the payload set.
    MissingFile(String),
    /// A payload file exists that the manifest does not describe.
    UnexpectedFile(String),
    /// The board reported by the device does not match the manifest.
    BoardMismatch {
        /// Board name reported by the device.
        device: String,
        /// Board name in the manifest.
        manifest: String,
    },
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::SizeMismatch {
                partition,
                declared,
                actual,
            } => write!(
                f,
                "partition {partition}: declared {declared} bytes but got {actual}"
            ),
            ImageError::ChecksumMismatch {
                partition,
                declared,
                actual,
            } => write!(
                f,
                "partition {partition}: checksum 0x{declared:08x} does not match computed 0x{actual:08x}"
            ),
            ImageError::Md5Mismatch { declared, actual } => {
                write!(f, "firmware-md5 {declared} does not match computed {actual}")
            }
            ImageError::InvalidNordicFirmware(why) => {
                write!(f, "Invalid Nordic firmware. ({why})")
            }
            ImageError::MissingFile(name) => write!(f, "missing firmware file {name}"),
            ImageError::UnexpectedFile(name) => {
                write!(f, "firmware file {name} is not described by the manifest")
            }
            ImageError::BoardMismatch { device, manifest } => write!(
                f,
                "device board {device} does not match manifest board {manifest}"
            ),
        }
    }
}

impl From<ManifestError> for Error {
    fn from(e: ManifestError) -> Self {
        Error::Manifest(e)
    }
}

impl From<HexError> for Error {
    fn from(e: HexError) -> Self {
        Error::Hex(e)
    }
}

impl From<VersionError> for Error {
    fn from(e: VersionError) -> Self {
        Error::Version(e)
    }
}

impl From<JsonError> for Error {
    fn from(e: JsonError) -> Self {
        Error::Json(e)
    }
}

impl From<ImageError> for Error {
    fn from(e: ImageError) -> Self {
        Error::Image(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Manifest(e) => write!(f, "manifest error: {e}"),
            Error::Hex(e) => write!(f, "hex error: {e}"),
            Error::Version(e) => write!(f, "version error: {e}"),
            Error::Json(e) => write!(f, "json error: {e}"),
            Error::Image(e) => write!(f, "image error: {e}"),
            Error::Download(e) => write!(f, "download error: {e}"),
            Error::Transport(e) => write!(f, "transport error: {e}"),
            Error::Cancelled => write!(f, "operation cancelled"),
        }
    }
}

impl std::error::Error for Error {}

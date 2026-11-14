//! RØDE Central firmware-update subsystem, reimplemented in Rust.
//!
//! The shipped application splits this feature across two artifacts:
//!
//! * `app.so` (Dart AOT) — UI, use cases, cubits, persistence keys and the
//!   user-visible [`FirmwareUpdateStatus`] values.
//! * `rode_bridge.dll` (C++/JUCE) — the actual firmware engine: an
//!   `FirmwareUpdater` hierarchy, an OTA manifest format, Intel HEX decoding,
//!   CRC/MD5 verification and the manifest downloader.
//!
//! This crate reconstructs the parts of that engine that are observable in the
//! shipped artifacts: the manifest and image formats, the verification
//! algorithms, version handling, the device-family dispatch table and the
//! orchestration state machine. Byte-level device transports cannot be
//! recovered from a stripped binary and are isolated behind
//! [`FirmwareTransport`]; see `docs/GAPS.md`.
//!
//! The crate is dependency-free so it builds offline.
//!
//! ```
//! use rode_fw::{FwVersion, OtaManifest, crc32};
//!
//! let m = OtaManifest::parse(rode_fw::fixtures::WGO2_RX_1_93).unwrap();
//! assert_eq!(m.version.board_name, "wgo2_rx");
//! assert_eq!(m.partitions.len(), 2);
//! assert_eq!(m.version.version.name, "1.93");
//!
//! // `<version_code>` is the decimal digits of `version_name` read as hex.
//! assert_eq!(FwVersion::from_name("1.93").unwrap().code, 0x193);
//! assert_eq!(crc32::crc32(b"123456789"), 0xcbf43926);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod cli;
pub mod crc32;
pub mod device;
pub mod download;
pub mod error;
pub mod fixtures;
pub mod hex;
pub mod json;
pub mod manager;
pub mod md5;
pub mod ota_xml;
pub mod persistence;
pub mod state;
pub mod version;

pub use device::{BoardProfile, DeviceFamily, UpdaterKind};
pub use download::{DownloadError, Downloader, FirmwareManifest, HttpFileDownloader};
pub use error::{Error, ImageError, Result};
pub use hex::{HexError, HexImage, HexRecord};
pub use json::{Json, JsonError};
pub use manager::{
    check_firmware, validate_layout, verify_payloads, DeviceInfo, EventLog, EventSink,
    FirmwareRevision, FirmwareTransport, FirmwareUpdateManager, NullSink, PartitionOutcome,
    PayloadSet, PlanDecision, SimulatedOp, SimulatedTransport, UpdateEvent, UpdateGate,
    UpdateOutcome, UpdatePlan, UpdatePolicy, UpdateReason, WRITE_CHUNK,
};
pub use ota_xml::{ManifestError, OtaManifest, OtaVersion, Partition, PartitionType, XmlStyle};
pub use persistence::{AnalyticsEvent, PersistedUpdateState, PersistenceKey};
pub use state::{FirmwareUpdateStatus, FlashMode, Phase, Progress, UpdateStateMachine};
pub use version::{FwVersion, VersionError};

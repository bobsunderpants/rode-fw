//! Device families, updater classes and the board → updater dispatch table.
//!
//! The updater hierarchy is visible in `rode_bridge.dll` through MSVC RTTI
//! type descriptors:
//!
//! ```text
//! FirmwareUpdater
//! ├── IntProFirmwareUpdater
//! ├── WirelessFirmwareUpdater
//! │   ├── WGo2FirmwareUpdater
//! │   ├── WGo3FirmwareUpdater
//! │   ├── WMeFirmwareUpdater
//! │   ├── WProFirmwareUpdater
//! │   ├── WMicroFirmwareUpdater
//! │   └── FilmmakerIIFirmwareUpdater
//! ├── DefaultFirmwareUpdater
//! ├── GenericFirmwareUpdater
//! ├── OtaFirmwareUpdater
//! └── QccHidFirmwareUpdater
//! ```
//!
//! The signal signatures attached to each class are also recoverable from
//! sigslot RTTI (`slot_pmf` type descriptors), and are recorded here in
//! [`Signals`].

use std::fmt;

/// Which signal slots a class instantiates, as recovered from sigslot RTTI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signals {
    /// `void(int)` — integer progress.
    pub progress: bool,
    /// `void(bool)` — completion flag.
    pub completion: bool,
    /// `void()` — argument-less "done".
    pub done: bool,
}

impl Signals {
    const fn new(progress: bool, completion: bool, done: bool) -> Self {
        Signals {
            progress,
            completion,
            done,
        }
    }
}

/// The concrete updater implementation the bridge instantiates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpdaterKind {
    /// `IntProFirmwareUpdater`
    IntPro,
    /// `WirelessFirmwareUpdater`
    Wireless,
    /// `WGo2FirmwareUpdater`
    WGo2,
    /// `WGo3FirmwareUpdater`
    WGo3,
    /// `WMeFirmwareUpdater`
    WMe,
    /// `WProFirmwareUpdater`
    WPro,
    /// `WMicroFirmwareUpdater`
    WMicro,
    /// `FilmmakerIIFirmwareUpdater`
    FilmmakerII,
    /// `DefaultFirmwareUpdater`
    Default,
    /// `GenericFirmwareUpdater`
    Generic,
    /// `OtaFirmwareUpdater`
    Ota,
    /// `QccHidFirmwareUpdater`
    QccHid,
}

impl UpdaterKind {
    /// Every kind, in hierarchy order.
    pub const ALL: [UpdaterKind; 12] = [
        UpdaterKind::IntPro,
        UpdaterKind::Wireless,
        UpdaterKind::WGo2,
        UpdaterKind::WGo3,
        UpdaterKind::WMe,
        UpdaterKind::WPro,
        UpdaterKind::WMicro,
        UpdaterKind::FilmmakerII,
        UpdaterKind::Default,
        UpdaterKind::Generic,
        UpdaterKind::Ota,
        UpdaterKind::QccHid,
    ];

    /// The RTTI/Dart-facing class name.
    pub fn class_name(self) -> &'static str {
        match self {
            UpdaterKind::IntPro => "IntProFirmwareUpdater",
            UpdaterKind::Wireless => "WirelessFirmwareUpdater",
            UpdaterKind::WGo2 => "WGo2FirmwareUpdater",
            UpdaterKind::WGo3 => "WGo3FirmwareUpdater",
            UpdaterKind::WMe => "WMeFirmwareUpdater",
            UpdaterKind::WPro => "WProFirmwareUpdater",
            UpdaterKind::WMicro => "WMicroFirmwareUpdater",
            UpdaterKind::FilmmakerII => "FilmmakerIIFirmwareUpdater",
            UpdaterKind::Default => "DefaultFirmwareUpdater",
            UpdaterKind::Generic => "GenericFirmwareUpdater",
            UpdaterKind::Ota => "OtaFirmwareUpdater",
            UpdaterKind::QccHid => "QccHidFirmwareUpdater",
        }
    }

    /// Signal slots instantiated for this class.
    pub fn signals(self) -> Signals {
        match self {
            // Only `void(int)` is present for Ota.
            UpdaterKind::Ota => Signals::new(true, false, false),
            // Generic adds the argument-less completion signal.
            UpdaterKind::Generic => Signals::new(true, true, true),
            // The remaining concrete updaters expose `void(int)` and `void(bool)`.
            _ => Signals::new(true, true, false),
        }
    }

    /// True when the class derives from `WirelessFirmwareUpdater`.
    pub fn is_wireless(self) -> bool {
        matches!(
            self,
            UpdaterKind::Wireless
                | UpdaterKind::WGo2
                | UpdaterKind::WGo3
                | UpdaterKind::WMe
                | UpdaterKind::WPro
                | UpdaterKind::WMicro
                | UpdaterKind::FilmmakerII
        )
    }

    /// True when the class consumes an OTA manifest with per-partition CRC.
    pub fn uses_ota_manifest(self) -> bool {
        matches!(
            self,
            UpdaterKind::WGo2
                | UpdaterKind::WGo3
                | UpdaterKind::WMe
                | UpdaterKind::WPro
                | UpdaterKind::WMicro
                | UpdaterKind::FilmmakerII
                | UpdaterKind::Wireless
                | UpdaterKind::Ota
        )
    }
}

impl fmt::Display for UpdaterKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.class_name())
    }
}

/// Coarse product families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceFamily {
    /// Wireless GO II (`wgo2_*`).
    WirelessGo2,
    /// Wireless GO II revision using the `wgo25_*` board ids.
    WirelessGo2Rev,
    /// Wireless ME (`wme_*`).
    WirelessMe,
    /// Wireless PRO.
    WirelessPro,
    /// Wireless MICRO.
    WirelessMicro,
    /// Filmmaker Kit II.
    Filmmaker2,
    /// Interview PRO.
    IntPro,
    /// The `wm_*` / `wmcam_*` board family (four partitions, `storage_id`).
    Wm,
    /// Qualcomm-based USB microphones (NT-USB+, PodMic USB).
    Qcc,
    /// Everything else driven by the default/generic updater.
    Generic,
}

impl DeviceFamily {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            DeviceFamily::WirelessGo2 => "Wireless GO II",
            DeviceFamily::WirelessGo2Rev => "Wireless GO II (2025 board)",
            DeviceFamily::WirelessMe => "Wireless ME",
            DeviceFamily::WirelessPro => "Wireless PRO",
            DeviceFamily::WirelessMicro => "Wireless MICRO",
            DeviceFamily::Filmmaker2 => "Filmmaker Kit II",
            DeviceFamily::IntPro => "Interview PRO",
            DeviceFamily::Wm => "WM series",
            DeviceFamily::Qcc => "Qualcomm USB microphone",
            DeviceFamily::Generic => "Generic",
        }
    }

    /// True when images for this family are Nordic (nRF) builds, i.e. use
    /// `mbrec.bin` + `zephyr.bin` and are validated as Intel HEX/DFU.
    pub fn is_nordic(self) -> bool {
        matches!(
            self,
            DeviceFamily::WirelessGo2
                | DeviceFamily::WirelessGo2Rev
                | DeviceFamily::WirelessMe
                | DeviceFamily::WirelessPro
                | DeviceFamily::WirelessMicro
                | DeviceFamily::Filmmaker2
        )
    }
}

/// How firmly a mapping is supported by the artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    /// Directly observed (RTTI class, manifest field, string).
    High,
    /// Strongly implied but one link is inferred.
    Medium,
    /// Best-effort; documented in `docs/GAPS.md`.
    Low,
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
        })
    }
}

/// Static description of a firmware board id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardProfile {
    /// `<board_name>` value, e.g. `wgo2_rx`.
    pub board: &'static str,
    /// Coarse product family.
    pub family: DeviceFamily,
    /// Updater class the bridge instantiates for this board.
    pub updater: UpdaterKind,
    /// How firm the `family`/`updater` mapping is.
    pub confidence: Confidence,
    /// Why this mapping was chosen.
    pub note: &'static str,
}

const fn profile(
    board: &'static str,
    family: DeviceFamily,
    updater: UpdaterKind,
    confidence: Confidence,
    note: &'static str,
) -> BoardProfile {
    BoardProfile {
        board,
        family,
        updater,
        confidence,
        note,
    }
}

/// Every board id that appears in the embedded manifests.
pub const BOARD_PROFILES: &[BoardProfile] = &[
    profile(
        "wgo2_rx",
        DeviceFamily::WirelessGo2,
        UpdaterKind::WGo2,
        Confidence::High,
        "board prefix matches WGo2FirmwareUpdater; Nordic layout (BOOT mbrec.bin + SYSTEM zephyr.bin)",
    ),
    profile(
        "wgo2_tx",
        DeviceFamily::WirelessGo2,
        UpdaterKind::WGo2,
        Confidence::High,
        "board prefix matches WGo2FirmwareUpdater; Nordic layout",
    ),
    profile(
        "wgo25_rx",
        DeviceFamily::WirelessGo2Rev,
        UpdaterKind::WGo2,
        Confidence::Low,
        "same version and partition layout as wgo2_rx but a different image; the 25 suffix is not explained by any recovered symbol",
    ),
    profile(
        "wgo25_tx",
        DeviceFamily::WirelessGo2Rev,
        UpdaterKind::WGo2,
        Confidence::Low,
        "same as wgo25_rx: distinct image at v2.50, updater class unresolved",
    ),
    profile(
        "wme_rx",
        DeviceFamily::WirelessMe,
        UpdaterKind::WMe,
        Confidence::High,
        "board prefix matches WMeFirmwareUpdater; Nordic layout",
    ),
    profile(
        "wme_tx",
        DeviceFamily::WirelessMe,
        UpdaterKind::WMe,
        Confidence::High,
        "board prefix matches WMeFirmwareUpdater; Nordic layout",
    ),
    profile(
        "wm_tx",
        DeviceFamily::Wm,
        UpdaterKind::Default,
        Confidence::Low,
        "four partitions with storage_id; no updater class name matches the wm prefix",
    ),
    profile(
        "wm_rx",
        DeviceFamily::Wm,
        UpdaterKind::Default,
        Confidence::Low,
        "four partitions with storage_id; updater class unresolved",
    ),
    profile(
        "wm_crx",
        DeviceFamily::Wm,
        UpdaterKind::Default,
        Confidence::Low,
        "camera-receiver variant of the wm family",
    ),
    profile(
        "wmcam_rx",
        DeviceFamily::Wm,
        UpdaterKind::Default,
        Confidence::Low,
        "camera-receiver variant of the wm family",
    ),
];

/// Looks up a board id.
pub fn board_profile(board: &str) -> Option<&'static BoardProfile> {
    BOARD_PROFILES.iter().find(|p| p.board == board)
}

/// Resolves the updater class for a board, falling back to the default
/// implementation.
pub fn resolve_updater(board: &str) -> UpdaterKind {
    board_profile(board)
        .map(|p| p.updater)
        .unwrap_or(UpdaterKind::Default)
}

/// True when a board's images should be validated as Nordic firmware.
pub fn is_nordic_board(board: &str) -> bool {
    board_profile(board)
        .map(|p| p.family.is_nordic())
        .unwrap_or(false)
}

/// USB product strings seen in `rode_bridge.dll`, mapped to a family.
///
/// These are the names the Windows HID/USB enumerator matches. `\VID_19F7` is
/// RØDE's vendor id; the product string table lives next to `\\?\hid#` and
/// `0HIDClass`.
pub const PRODUCT_STRINGS: &[(&str, DeviceFamily)] = &[
    ("NT-USB", DeviceFamily::Generic),
    ("NT-USB Mini", DeviceFamily::Generic),
    ("NT-USB+", DeviceFamily::Qcc),
    ("PodMic USB", DeviceFamily::Qcc),
    ("VideoMic NTG", DeviceFamily::Generic),
    ("Wireless GO", DeviceFamily::WirelessGo2),
    ("Wireless GO II", DeviceFamily::WirelessGo2),
    ("Wireless ME", DeviceFamily::WirelessMe),
    ("Wireless PRO", DeviceFamily::WirelessPro),
    ("Wireless MICRO", DeviceFamily::WirelessMicro),
    ("Interview PRO", DeviceFamily::IntPro),
    ("XDM-100", DeviceFamily::Generic),
    ("RØDECaster", DeviceFamily::IntPro),
];

/// RØDE USB vendor id observed as `VID_19F7`.
pub const RODE_USB_VID: u16 = 0x19f7;

/// Matches a USB product string to a family (exact, then prefix).
pub fn product_family(product: &str) -> Option<DeviceFamily> {
    let p = product.trim();
    if let Some((_, family)) = PRODUCT_STRINGS.iter().find(|(name, _)| *name == p) {
        return Some(*family);
    }
    PRODUCT_STRINGS
        .iter()
        .find(|(name, _)| p.starts_with(name))
        .map(|(_, family)| *family)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ota_xml::OtaManifest;

    #[test]
    fn every_manifest_board_has_a_profile() {
        for (board, xml) in crate::fixtures::ALL {
            let m = OtaManifest::parse(xml).unwrap();
            let p =
                board_profile(m.board_name()).unwrap_or_else(|| panic!("no profile for {board}"));
            assert_eq!(p.board, *board);
        }
    }

    #[test]
    fn updater_hierarchy_is_consistent() {
        for kind in UpdaterKind::ALL {
            assert!(!kind.class_name().is_empty());
            let sig = kind.signals();
            assert!(sig.progress);
        }
        assert!(UpdaterKind::WGo2.is_wireless());
        assert!(!UpdaterKind::QccHid.is_wireless());
        assert!(UpdaterKind::Generic.signals().done);
        assert!(!UpdaterKind::WGo2.signals().done);
        assert!(!UpdaterKind::Ota.signals().completion);
    }

    #[test]
    fn nordic_boards() {
        assert!(is_nordic_board("wgo2_rx"));
        assert!(is_nordic_board("wme_tx"));
        assert!(!is_nordic_board("wm_tx"));
        assert!(!is_nordic_board("nonsense"));
    }

    #[test]
    fn product_matching() {
        assert_eq!(product_family("NT-USB+"), Some(DeviceFamily::Qcc));
        assert_eq!(product_family("NT-USB Mini"), Some(DeviceFamily::Generic));
        assert_eq!(
            product_family("Wireless GO II"),
            Some(DeviceFamily::WirelessGo2)
        );
        assert_eq!(product_family("Unknown Thing"), None);
    }

    #[test]
    fn unknown_board_falls_back_to_default_updater() {
        assert_eq!(resolve_updater("zzz"), UpdaterKind::Default);
    }
}

//! Real OTA manifests recovered from `rode_bridge.dll`.
//!
//! These are the twelve `ota_firmware` documents embedded in the shipped
//! binary's `.rdata`. They are used as test vectors and by `rode-fw manifest
//! fixtures` so the parser can be exercised against genuine data rather than
//! hand-written samples.
//!
//! Source addresses (image base `0x180000000`):
//!
//! | board | version | address |
//! |-------|---------|---------|
//! | `wgo2_rx`  | 1.93 | `0x18068bd94` |
//! | `wgo2_tx`  | 1.93 | `0x18070d5c6` |
//! | `wgo2_rx`  | 2.50 | `0x1807b26b1` |
//! | `wgo2_tx`  | 2.50 | `0x18085e0eb` |
//! | `wgo25_rx` | 2.50 | `0x18090c9b2` |
//! | `wgo25_tx` | 2.50 | `0x1809b77ed` |
//! | `wme_rx`   | 1.47 | `0x181056eb0` |
//! | `wme_tx`   | 1.47 | `0x1810b40e9` |
//! | `wm_tx`    | 2.31 | `0x1815e7d8d` |
//! | `wm_rx`    | 2.31 | `0x1816a19ca` |
//! | `wm_crx`   | 2.31 | `0x181753208` |
//! | `wmcam_rx` | 2.31 | `0x181808648` |

/// `wgo2_rx`, version 1.93, tab-indented serialisation.
pub const WGO2_RX_1_93: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "\t<firmware_version>\n",
    "\t\t<board_name>wgo2_rx</board_name>\n",
    "\t\t<version_name>1.93</version_name>\n",
    "\t\t<version_code>0x193</version_code>\n",
    "\t\t</firmware_version>\n",
    "<partitions>\n",
    "\t<partitionsNum>2</partitionsNum>\n",
    "\t<partition>\n",
    "\t\t<type>BOOT</type>\n",
    "\t\t<name>fw0_boot</name>\n",
    "\t\t<file_id>1</file_id>\n",
    "\t\t<file_name>mbrec.bin</file_name>\n",
    "\t\t<file_size>0x1000</file_size>\n",
    "\t\t<checksum>0x488ed61c</checksum>\n",
    "\t</partition>\n",
    "\t<partition>\n",
    "\t\t<type>SYSTEM</type>\n",
    "\t\t<name>fw0_sys</name>\n",
    "\t\t<file_id>3</file_id>\n",
    "\t\t<file_name>zephyr.bin</file_name>\n",
    "\t\t<file_size>0x7fee8</file_size>\n",
    "\t\t<checksum>0x2bbd06d7</checksum>\n",
    "\t</partition>\n",
    "\t</partitions>\n",
    "\t</ota_firmware>\n",
);

/// `wgo2_tx`, version 1.93.
pub const WGO2_TX_1_93: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "\t<firmware_version>\n",
    "\t\t<board_name>wgo2_tx</board_name>\n",
    "\t\t<version_name>1.93</version_name>\n",
    "\t\t<version_code>0x193</version_code>\n",
    "\t\t</firmware_version>\n",
    "<partitions>\n",
    "\t<partitionsNum>2</partitionsNum>\n",
    "\t<partition>\n",
    "\t\t<type>BOOT</type>\n",
    "\t\t<name>fw0_boot</name>\n",
    "\t\t<file_id>1</file_id>\n",
    "\t\t<file_name>mbrec.bin</file_name>\n",
    "\t\t<file_size>0x1000</file_size>\n",
    "\t\t<checksum>0x240a9296</checksum>\n",
    "\t</partition>\n",
    "\t<partition>\n",
    "\t\t<type>SYSTEM</type>\n",
    "\t\t<name>fw0_sys</name>\n",
    "\t\t<file_id>3</file_id>\n",
    "\t\t<file_name>zephyr.bin</file_name>\n",
    "\t\t<file_size>0x40f64</file_size>\n",
    "\t\t<checksum>0xf0b8b437</checksum>\n",
    "\t</partition>\n",
    "\t</partitions>\n",
    "\t</ota_firmware>\n",
);

/// `wgo2_rx`, version 2.50.
pub const WGO2_RX_2_50: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "\t<firmware_version>\n",
    "\t\t<board_name>wgo2_rx</board_name>\n",
    "\t\t<version_name>2.50</version_name>\n",
    "\t\t<version_code>0x250</version_code>\n",
    "\t\t</firmware_version>\n",
    "<partitions>\n",
    "\t<partitionsNum>2</partitionsNum>\n",
    "\t<partition>\n",
    "\t\t<type>BOOT</type>\n",
    "\t\t<name>fw0_boot</name>\n",
    "\t\t<file_id>1</file_id>\n",
    "\t\t<file_name>mbrec.bin</file_name>\n",
    "\t\t<file_size>0x1000</file_size>\n",
    "\t\t<checksum>0x894d9dde</checksum>\n",
    "\t</partition>\n",
    "\t<partition>\n",
    "\t\t<type>SYSTEM</type>\n",
    "\t\t<name>fw0_sys</name>\n",
    "\t\t<file_id>3</file_id>\n",
    "\t\t<file_name>zephyr.bin</file_name>\n",
    "\t\t<file_size>0xaa198</file_size>\n",
    "\t\t<checksum>0x4d051b4f</checksum>\n",
    "\t</partition>\n",
    "\t</partitions>\n",
    "\t</ota_firmware>\n",
);

/// `wgo2_tx`, version 2.50.
pub const WGO2_TX_2_50: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "\t<firmware_version>\n",
    "\t\t<board_name>wgo2_tx</board_name>\n",
    "\t\t<version_name>2.50</version_name>\n",
    "\t\t<version_code>0x250</version_code>\n",
    "\t\t</firmware_version>\n",
    "<partitions>\n",
    "\t<partitionsNum>2</partitionsNum>\n",
    "\t<partition>\n",
    "\t\t<type>BOOT</type>\n",
    "\t\t<name>fw0_boot</name>\n",
    "\t\t<file_id>1</file_id>\n",
    "\t\t<file_name>mbrec.bin</file_name>\n",
    "\t\t<file_size>0x1000</file_size>\n",
    "\t\t<checksum>0xbf2c2d11</checksum>\n",
    "\t</partition>\n",
    "\t<partition>\n",
    "\t\t<type>SYSTEM</type>\n",
    "\t\t<name>fw0_sys</name>\n",
    "\t\t<file_id>3</file_id>\n",
    "\t\t<file_name>zephyr.bin</file_name>\n",
    "\t\t<file_size>0x60c76</file_size>\n",
    "\t\t<checksum>0xdd46fb66</checksum>\n",
    "\t</partition>\n",
    "\t</partitions>\n",
    "\t</ota_firmware>\n",
);

/// `wgo25_rx`, version 2.50.
pub const WGO25_RX_2_50: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "\t<firmware_version>\n",
    "\t\t<board_name>wgo25_rx</board_name>\n",
    "\t\t<version_name>2.50</version_name>\n",
    "\t\t<version_code>0x250</version_code>\n",
    "\t\t</firmware_version>\n",
    "<partitions>\n",
    "\t<partitionsNum>2</partitionsNum>\n",
    "\t<partition>\n",
    "\t\t<type>BOOT</type>\n",
    "\t\t<name>fw0_boot</name>\n",
    "\t\t<file_id>1</file_id>\n",
    "\t\t<file_name>mbrec.bin</file_name>\n",
    "\t\t<file_size>0x1000</file_size>\n",
    "\t\t<checksum>0x7882008f</checksum>\n",
    "\t</partition>\n",
    "\t<partition>\n",
    "\t\t<type>SYSTEM</type>\n",
    "\t\t<name>fw0_sys</name>\n",
    "\t\t<file_id>3</file_id>\n",
    "\t\t<file_name>zephyr.bin</file_name>\n",
    "\t\t<file_size>0xa951c</file_size>\n",
    "\t\t<checksum>0xb4662a02</checksum>\n",
    "\t</partition>\n",
    "\t</partitions>\n",
    "\t</ota_firmware>\n",
);

/// `wgo25_tx`, version 2.50.
pub const WGO25_TX_2_50: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "\t<firmware_version>\n",
    "\t\t<board_name>wgo25_tx</board_name>\n",
    "\t\t<version_name>2.50</version_name>\n",
    "\t\t<version_code>0x250</version_code>\n",
    "\t\t</firmware_version>\n",
    "<partitions>\n",
    "\t<partitionsNum>2</partitionsNum>\n",
    "\t<partition>\n",
    "\t\t<type>BOOT</type>\n",
    "\t\t<name>fw0_boot</name>\n",
    "\t\t<file_id>1</file_id>\n",
    "\t\t<file_name>mbrec.bin</file_name>\n",
    "\t\t<file_size>0x1000</file_size>\n",
    "\t\t<checksum>0x42d0822f</checksum>\n",
    "\t</partition>\n",
    "\t<partition>\n",
    "\t\t<type>SYSTEM</type>\n",
    "\t\t<name>fw0_sys</name>\n",
    "\t\t<file_id>3</file_id>\n",
    "\t\t<file_name>zephyr.bin</file_name>\n",
    "\t\t<file_size>0x5fea6</file_size>\n",
    "\t\t<checksum>0xff4a1799</checksum>\n",
    "\t</partition>\n",
    "\t</partitions>\n",
    "\t</ota_firmware>\n",
);

/// `wme_rx`, version 1.47.
pub const WME_RX_1_47: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "\t<firmware_version>\n",
    "\t\t<board_name>wme_rx</board_name>\n",
    "\t\t<version_name>1.47</version_name>\n",
    "\t\t<version_code>0x147</version_code>\n",
    "\t\t</firmware_version>\n",
    "<partitions>\n",
    "\t<partitionsNum>2</partitionsNum>\n",
    "\t<partition>\n",
    "\t\t<type>BOOT</type>\n",
    "\t\t<name>fw0_boot</name>\n",
    "\t\t<file_id>1</file_id>\n",
    "\t\t<file_name>mbrec.bin</file_name>\n",
    "\t\t<file_size>0x1000</file_size>\n",
    "\t\t<checksum>0xf77ce7d8</checksum>\n",
    "\t</partition>\n",
    "\t<partition>\n",
    "\t\t<type>SYSTEM</type>\n",
    "\t\t<name>fw0_sys</name>\n",
    "\t\t<file_id>3</file_id>\n",
    "\t\t<file_name>zephyr.bin</file_name>\n",
    "\t\t<file_size>0x5b9da</file_size>\n",
    "\t\t<checksum>0x4cecfd87</checksum>\n",
    "\t</partition>\n",
    "\t</partitions>\n",
    "\t</ota_firmware>\n",
);

/// `wme_tx`, version 1.47.
pub const WME_TX_1_47: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "\t<firmware_version>\n",
    "\t\t<board_name>wme_tx</board_name>\n",
    "\t\t<version_name>1.47</version_name>\n",
    "\t\t<version_code>0x147</version_code>\n",
    "\t\t</firmware_version>\n",
    "<partitions>\n",
    "\t<partitionsNum>2</partitionsNum>\n",
    "\t<partition>\n",
    "\t\t<type>BOOT</type>\n",
    "\t\t<name>fw0_boot</name>\n",
    "\t\t<file_id>1</file_id>\n",
    "\t\t<file_name>mbrec.bin</file_name>\n",
    "\t\t<file_size>0x1000</file_size>\n",
    "\t\t<checksum>0xf57b7a06</checksum>\n",
    "\t</partition>\n",
    "\t<partition>\n",
    "\t\t<type>SYSTEM</type>\n",
    "\t\t<name>fw0_sys</name>\n",
    "\t\t<file_id>3</file_id>\n",
    "\t\t<file_name>zephyr.bin</file_name>\n",
    "\t\t<file_size>0x57ae6</file_size>\n",
    "\t\t<checksum>0xb5736489</checksum>\n",
    "\t</partition>\n",
    "\t</partitions>\n",
    "\t</ota_firmware>\n",
);

/// `wm_tx`, version 2.31, compact serialisation.
pub const WM_TX_2_31: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "<firmware_version>\n",
    "<version_code>0x231</version_code>\n",
    "<version_name>2.31</version_name>\n",
    "<version_res />\n",
    "<board_name>wm_tx</board_name>\n",
    "</firmware_version>\n",
    "<partitions>\n",
    "<partitionsNum>4</partitionsNum>\n",
    "<partition>\n",
    "<type>SYSTEM</type>\n",
    "<name>fw0_sys</name>\n",
    "<file_id>4</file_id>\n",
    "<file_name>app.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x9ca60</file_size>\n",
    "<checksum>0x5dc312b3</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>DATA</type>\n",
    "<name>fw0_sdfs</name>\n",
    "<file_id>39</file_id>\n",
    "<file_name>tone.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x1b1a0</file_size>\n",
    "<checksum>0xd81bef6b</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>BOOT</type>\n",
    "<name>fw0_boot</name>\n",
    "<file_id>1</file_id>\n",
    "<file_name>mbrec.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x1000</file_size>\n",
    "<checksum>0xe4d27a38</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>SYS_PARAM</type>\n",
    "<name>fw0_param</name>\n",
    "<file_id>2</file_id>\n",
    "<file_name>param.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x400</file_size>\n",
    "<checksum>0x6debcf49</checksum>\n",
    "</partition>\n",
    "</partitions>\n",
    "</ota_firmware>\n",
);

/// `wm_rx`, version 2.31, compact serialisation.
pub const WM_RX_2_31: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "<firmware_version>\n",
    "<version_code>0x231</version_code>\n",
    "<version_name>2.31</version_name>\n",
    "<version_res />\n",
    "<board_name>wm_rx</board_name>\n",
    "</firmware_version>\n",
    "<partitions>\n",
    "<partitionsNum>4</partitionsNum>\n",
    "<partition>\n",
    "<type>SYSTEM</type>\n",
    "<name>fw0_sys</name>\n",
    "<file_id>4</file_id>\n",
    "<file_name>app.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x94720</file_size>\n",
    "<checksum>0xeba5d250</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>DATA</type>\n",
    "<name>fw0_sdfs</name>\n",
    "<file_id>39</file_id>\n",
    "<file_name>tone.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x1b1a0</file_size>\n",
    "<checksum>0xd81bef6b</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>BOOT</type>\n",
    "<name>fw0_boot</name>\n",
    "<file_id>1</file_id>\n",
    "<file_name>mbrec.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x1000</file_size>\n",
    "<checksum>0xe4d27a38</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>SYS_PARAM</type>\n",
    "<name>fw0_param</name>\n",
    "<file_id>2</file_id>\n",
    "<file_name>param.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x400</file_size>\n",
    "<checksum>0x2575794a</checksum>\n",
    "</partition>\n",
    "</partitions>\n",
    "</ota_firmware>\n",
);

/// `wm_crx`, version 2.31, compact serialisation.
pub const WM_CRX_2_31: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "<firmware_version>\n",
    "<version_code>0x231</version_code>\n",
    "<version_name>2.31</version_name>\n",
    "<version_res />\n",
    "<board_name>wm_crx</board_name>\n",
    "</firmware_version>\n",
    "<partitions>\n",
    "<partitionsNum>4</partitionsNum>\n",
    "<partition>\n",
    "<type>SYSTEM</type>\n",
    "<name>fw0_sys</name>\n",
    "<file_id>4</file_id>\n",
    "<file_name>app.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x98240</file_size>\n",
    "<checksum>0x7d57af20</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>DATA</type>\n",
    "<name>fw0_sdfs</name>\n",
    "<file_id>39</file_id>\n",
    "<file_name>tone.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x1b1a0</file_size>\n",
    "<checksum>0xd81bef6b</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>BOOT</type>\n",
    "<name>fw0_boot</name>\n",
    "<file_id>1</file_id>\n",
    "<file_name>mbrec.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x1000</file_size>\n",
    "<checksum>0xe4d27a38</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>SYS_PARAM</type>\n",
    "<name>fw0_param</name>\n",
    "<file_id>2</file_id>\n",
    "<file_name>param.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x400</file_size>\n",
    "<checksum>0x26d8a5a8</checksum>\n",
    "</partition>\n",
    "</partitions>\n",
    "</ota_firmware>\n",
);

/// `wmcam_rx`, version 2.31, compact serialisation.
pub const WMCAM_RX_2_31: &str = concat!(
    "<?xml version='1.0' encoding='UTF-8'?>\n",
    "<ota_firmware>\n",
    "<firmware_version>\n",
    "<version_code>0x231</version_code>\n",
    "<version_name>2.31</version_name>\n",
    "<version_res />\n",
    "<board_name>wmcam_rx</board_name>\n",
    "</firmware_version>\n",
    "<partitions>\n",
    "<partitionsNum>4</partitionsNum>\n",
    "<partition>\n",
    "<type>SYSTEM</type>\n",
    "<name>fw0_sys</name>\n",
    "<file_id>4</file_id>\n",
    "<file_name>app.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x9e4e0</file_size>\n",
    "<checksum>0xed9ea80d</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>DATA</type>\n",
    "<name>fw0_sdfs</name>\n",
    "<file_id>39</file_id>\n",
    "<file_name>tone.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x1b1a0</file_size>\n",
    "<checksum>0xd81bef6b</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>BOOT</type>\n",
    "<name>fw0_boot</name>\n",
    "<file_id>1</file_id>\n",
    "<file_name>mbrec.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x1000</file_size>\n",
    "<checksum>0xe4d27a38</checksum>\n",
    "</partition>\n",
    "<partition>\n",
    "<type>SYS_PARAM</type>\n",
    "<name>fw0_param</name>\n",
    "<file_id>2</file_id>\n",
    "<file_name>param.bin</file_name>\n",
    "<storage_id>0</storage_id>\n",
    "<file_size>0x400</file_size>\n",
    "<checksum>0x52663166</checksum>\n",
    "</partition>\n",
    "</partitions>\n",
    "</ota_firmware>\n",
);

/// All twelve embedded manifests as `(board, xml)`.
pub const ALL: &[(&str, &str)] = &[
    ("wgo2_rx", WGO2_RX_1_93),
    ("wgo2_tx", WGO2_TX_1_93),
    ("wgo2_rx", WGO2_RX_2_50),
    ("wgo2_tx", WGO2_TX_2_50),
    ("wgo25_rx", WGO25_RX_2_50),
    ("wgo25_tx", WGO25_TX_2_50),
    ("wme_rx", WME_RX_1_47),
    ("wme_tx", WME_TX_1_47),
    ("wm_tx", WM_TX_2_31),
    ("wm_rx", WM_RX_2_31),
    ("wm_crx", WM_CRX_2_31),
    ("wmcam_rx", WMCAM_RX_2_31),
];

/// The ten tab-indented manifests.
pub const TABBED: &[(&str, &str)] = &[
    ("wgo2_rx", WGO2_RX_1_93),
    ("wgo2_tx", WGO2_TX_1_93),
    ("wgo2_rx", WGO2_RX_2_50),
    ("wgo2_tx", WGO2_TX_2_50),
    ("wgo25_rx", WGO25_RX_2_50),
    ("wgo25_tx", WGO25_TX_2_50),
    ("wme_rx", WME_RX_1_47),
    ("wme_tx", WME_TX_1_47),
];

/// The four compact manifests.
pub const COMPACT: &[(&str, &str)] = &[
    ("wm_tx", WM_TX_2_31),
    ("wm_rx", WM_RX_2_31),
    ("wm_crx", WM_CRX_2_31),
    ("wmcam_rx", WMCAM_RX_2_31),
];

/// Looks up an embedded manifest by board name and version.
pub fn find(board: &str, version: &str) -> Option<&'static str> {
    ALL.iter()
        .find(|(b, xml)| *b == board && OtaManifestVersion::of(xml) == version)
        .map(|(_, xml)| *xml)
}

struct OtaManifestVersion;

impl OtaManifestVersion {
    fn of(xml: &str) -> &str {
        // The fixtures are well-formed; avoid parsing cost by scanning.
        let start = match xml.find("<version_name>") {
            Some(i) => i + "<version_name>".len(),
            None => return "",
        };
        let end = match xml[start..].find("</version_name>") {
            Some(i) => start + i,
            None => return "",
        };
        &xml[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_consistent() {
        assert_eq!(ALL.len(), 12);
        assert_eq!(TABBED.len() + COMPACT.len(), ALL.len());
    }

    #[test]
    fn find_works() {
        assert!(find("wgo2_rx", "2.50").is_some());
        assert_eq!(find("wgo2_rx", "9.99"), None);
        assert_eq!(find("wm_crx", "2.31").unwrap(), WM_CRX_2_31);
    }

    #[test]
    fn fixtures_are_ascii_xml_ending_in_newline() {
        for (board, xml) in ALL {
            assert!(xml.is_ascii(), "{board}");
            assert!(xml.ends_with('\n'), "{board}");
            assert!(xml.starts_with("<?xml "), "{board}");
        }
    }
}

//! The OTA manifest (`ota_firmware` XML).
//!
//! Twelve manifests are embedded in `rode_bridge.dll`; they use two distinct
//! serialisations of the same schema. Ten are tab-indented with the order
//! `board_name, version_name, version_code`; four are compact with the order
//! `version_code, version_name, version_res, board_name` and an explicit
//! `<version_res />`. Both are accepted and both can be reproduced byte for
//! byte (see [`XmlStyle`]).
//!
//! ```
//! use rode_fw::ota_xml::{OtaManifest, PartitionType, XmlStyle};
//!
//! let text = rode_fw::fixtures::WM_TX_2_31;
//! let manifest = OtaManifest::parse(text).unwrap();
//! assert_eq!(manifest.partitions.len(), 4);
//! assert_eq!(manifest.partitions[0].ptype, PartitionType::System);
//! assert_eq!(manifest.to_xml(XmlStyle::Compact), text);
//! ```

use crate::version::{FwVersion, VersionError};
use std::fmt;

/// Serialisation style of an OTA manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmlStyle {
    /// Tab-indented, `board_name` first. Used by the Nordic (W Go / W ME)
    /// manifests.
    Tabbed,
    /// No indentation, `version_code` first, explicit `<version_res />`. Used
    /// by the `wm_*` manifests.
    Compact,
}

/// Partition classes used by the OTA manifest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PartitionType {
    /// Bootloader (`fw0_boot`, `file_id` 1).
    Boot,
    /// Main application (`fw0_sys`, `file_id` 3 or 4).
    System,
    /// Data/asset partition (`fw0_sdfs`, `file_id` 39).
    Data,
    /// System parameters (`fw0_param`, `file_id` 2).
    SysParam,
    /// Anything the parser does not know; preserved verbatim.
    Other(String),
}

impl PartitionType {
    /// Maps the `<type>` text to a partition class.
    pub fn parse(text: &str) -> PartitionType {
        match text.trim() {
            "BOOT" => PartitionType::Boot,
            "SYSTEM" => PartitionType::System,
            "DATA" => PartitionType::Data,
            "SYS_PARAM" => PartitionType::SysParam,
            other => PartitionType::Other(other.to_string()),
        }
    }

    /// The `<type>` text.
    pub fn as_str(&self) -> &str {
        match self {
            PartitionType::Boot => "BOOT",
            PartitionType::System => "SYSTEM",
            PartitionType::Data => "DATA",
            PartitionType::SysParam => "SYS_PARAM",
            PartitionType::Other(s) => s,
        }
    }
}

/// One `<partition>` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    /// `<type>`
    pub ptype: PartitionType,
    /// `<name>`, e.g. `fw0_boot`.
    pub name: String,
    /// `<file_id>`
    pub file_id: u32,
    /// `<file_name>`, e.g. `mbrec.bin`.
    pub file_name: String,
    /// `<storage_id>`; present only in the `wm_*` manifests.
    pub storage_id: Option<u32>,
    /// `<file_size>` after decompression/decoding.
    pub file_size: u32,
    /// `<checksum>`: CRC-32 of the decoded payload.
    pub checksum: u32,
}

/// The `<firmware_version>` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaVersion {
    /// `<board_name>`, e.g. `wgo2_rx`.
    pub board_name: String,
    /// `<version_name>` and `<version_code>`.
    pub version: FwVersion,
    /// `<version_res>` when present (empty in every shipped manifest).
    pub version_res: Option<String>,
}

/// A parsed `ota_firmware` document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaManifest {
    /// Target board and version.
    pub version: OtaVersion,
    /// `<partitionsNum>` exactly as shipped.
    pub partitions_num: u32,
    /// `<partition>` entries in document order.
    pub partitions: Vec<Partition>,
}

/// Manifest parse/validate failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// XML was structurally invalid.
    Xml(XmlError),
    /// The root element was not `ota_firmware`.
    UnexpectedRoot(String),
    /// A required element was absent.
    MissingElement(String),
    /// A numeric element could not be parsed.
    BadNumber {
        /// Element path, e.g. `partition/file_size`.
        field: String,
        /// Text that failed to parse.
        value: String,
    },
    /// `<partitionsNum>` disagrees with the number of `<partition>` elements.
    PartitionCountMismatch {
        /// Value of `<partitionsNum>`.
        declared: u32,
        /// Number of `<partition>` elements.
        actual: usize,
    },
    /// Two partitions share a `file_id`.
    DuplicateFileId(u32),
    /// Two partitions share a `name`.
    DuplicateName(String),
    /// `<partitions>` was empty.
    NoPartitions,
    /// `<file_size>` was zero.
    ZeroSize(String),
    /// The partition layout does not match the board's family.
    Layout(String),
    /// A version field was invalid.
    Version(VersionError),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Xml(e) => write!(f, "xml: {e}"),
            ManifestError::UnexpectedRoot(root) => {
                write!(
                    f,
                    "unexpected root element <{root}>, expected <ota_firmware>"
                )
            }
            ManifestError::MissingElement(path) => write!(f, "missing element {path}"),
            ManifestError::BadNumber { field, value } => {
                write!(f, "element {field} is not a number: {value:?}")
            }
            ManifestError::PartitionCountMismatch { declared, actual } => write!(
                f,
                "<partitionsNum> is {declared} but {actual} <partition> elements were found"
            ),
            ManifestError::DuplicateFileId(id) => write!(f, "duplicate <file_id> {id}"),
            ManifestError::DuplicateName(name) => write!(f, "duplicate partition name {name}"),
            ManifestError::NoPartitions => write!(f, "<partitions> contains no entries"),
            ManifestError::ZeroSize(name) => write!(f, "partition {name} declares size 0"),
            ManifestError::Layout(why) => write!(f, "invalid partition layout: {why}"),
            ManifestError::Version(e) => write!(f, "version: {e}"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl From<XmlError> for ManifestError {
    fn from(e: XmlError) -> Self {
        ManifestError::Xml(e)
    }
}

impl From<VersionError> for ManifestError {
    fn from(e: VersionError) -> Self {
        ManifestError::Version(e)
    }
}

/// Minimal XML failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlError {
    /// Input ended inside a tag.
    UnexpectedEnd,
    /// A closing tag did not match the open element.
    MismatchedTag {
        /// Element that was open.
        expected: String,
        /// Element named by the closing tag.
        found: String,
    },
    /// A closing tag had no matching open element.
    StrayClose(String),
    /// The document contained no element.
    Empty,
    /// An unterminated `<!--` comment or `<?` declaration.
    Unterminated,
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XmlError::UnexpectedEnd => write!(f, "unexpected end of XML"),
            XmlError::MismatchedTag { expected, found } => {
                write!(f, "expected </{expected}> but found </{found}>")
            }
            XmlError::StrayClose(name) => write!(f, "unexpected </{name}>"),
            XmlError::Empty => write!(f, "XML document has no root element"),
            XmlError::Unterminated => write!(f, "unterminated declaration or comment"),
        }
    }
}

impl std::error::Error for XmlError {}

impl OtaManifest {
    /// Parses and validates an `ota_firmware` document.
    pub fn parse(text: &str) -> std::result::Result<OtaManifest, ManifestError> {
        let root = parse_document(text)?;
        if root.name != "ota_firmware" {
            return Err(ManifestError::UnexpectedRoot(root.name));
        }

        let fw = root
            .child("firmware_version")
            .ok_or_else(|| ManifestError::MissingElement("firmware_version".into()))?;

        let board_name = fw.text_of("board_name")?.trim().to_string();
        if board_name.is_empty() {
            return Err(ManifestError::MissingElement(
                "firmware_version/board_name".into(),
            ));
        }
        let version_name = fw.text_of("version_name")?.trim().to_string();
        if version_name.is_empty() {
            return Err(ManifestError::MissingElement(
                "firmware_version/version_name".into(),
            ));
        }
        let code = number_u32(fw.text_of("version_code")?, "firmware_version/version_code")?;
        let version_res = fw.child("version_res").map(|_| String::new());
        let version = FwVersion::new(&version_name, code);
        // Validate the name is parseable even though the code is authoritative.
        version.segments()?;

        let partitions_el = root
            .child("partitions")
            .ok_or_else(|| ManifestError::MissingElement("partitions".into()))?;
        let partitions_num = number_u32(
            partitions_el.text_of("partitionsNum")?,
            "partitions/partitionsNum",
        )?;

        let mut partitions = Vec::new();
        for p in partitions_el.children_named("partition") {
            let ptype = PartitionType::parse(p.text_of("type")?);
            let name = p.text_of("name")?.trim().to_string();
            let file_id = number_u32(p.text_of("file_id")?, "partition/file_id")?;
            let file_name = p.text_of("file_name")?.trim().to_string();
            let storage_id = match p.child("storage_id") {
                Some(_) => Some(number_u32(
                    p.text_of("storage_id")?,
                    "partition/storage_id",
                )?),
                None => None,
            };
            let file_size = number_u32(p.text_of("file_size")?, "partition/file_size")?;
            let checksum = number_u32(p.text_of("checksum")?, "partition/checksum")?;
            partitions.push(Partition {
                ptype,
                name,
                file_id,
                file_name,
                storage_id,
                file_size,
                checksum,
            });
        }

        let manifest = OtaManifest {
            version: OtaVersion {
                board_name,
                version,
                version_res,
            },
            partitions_num,
            partitions,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Enforces the invariants the manifest format relies on.
    pub fn validate(&self) -> std::result::Result<(), ManifestError> {
        if self.partitions.is_empty() {
            return Err(ManifestError::NoPartitions);
        }
        if self.partitions_num as usize != self.partitions.len() {
            return Err(ManifestError::PartitionCountMismatch {
                declared: self.partitions_num,
                actual: self.partitions.len(),
            });
        }

        let mut ids = Vec::new();
        let mut names = Vec::new();
        for p in &self.partitions {
            if p.file_size == 0 {
                return Err(ManifestError::ZeroSize(p.name.clone()));
            }
            if ids.contains(&p.file_id) {
                return Err(ManifestError::DuplicateFileId(p.file_id));
            }
            if names.contains(&p.name) {
                return Err(ManifestError::DuplicateName(p.name.clone()));
            }
            ids.push(p.file_id);
            names.push(p.name.clone());
        }
        Ok(())
    }

    /// Board this manifest targets.
    pub fn board_name(&self) -> &str {
        &self.version.board_name
    }

    /// Target version.
    pub fn version(&self) -> &FwVersion {
        &self.version.version
    }

    /// True when `<version_code>` equals the code derived from
    /// `<version_name>` (see [`FwVersion::code_from_name`]).
    pub fn code_derivation_consistent(&self) -> bool {
        self.version.version.declared_code_matches()
    }

    /// First partition of a given class.
    pub fn partition(&self, ptype: PartitionType) -> Option<&Partition> {
        self.partitions.iter().find(|p| p.ptype == ptype)
    }

    /// Total decoded payload size across partitions.
    pub fn total_size(&self) -> u64 {
        self.partitions.iter().map(|p| p.file_size as u64).sum()
    }

    /// Distinct payload file names, in manifest order.
    pub fn payload_files(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for p in &self.partitions {
            if !out.contains(&p.file_name.as_str()) {
                out.push(&p.file_name);
            }
        }
        out
    }

    /// Renders the manifest in the requested style.
    pub fn to_xml(&self, style: XmlStyle) -> String {
        let mut out = String::from("<?xml version='1.0' encoding='UTF-8'?>\n");
        match style {
            XmlStyle::Tabbed => {
                out.push_str("<ota_firmware>\n");
                out.push_str("\t<firmware_version>\n");
                out.push_str(&format!(
                    "\t\t<board_name>{}</board_name>\n",
                    self.version.board_name
                ));
                out.push_str(&format!(
                    "\t\t<version_name>{}</version_name>\n",
                    self.version.version.name
                ));
                out.push_str(&format!(
                    "\t\t<version_code>0x{:x}</version_code>\n",
                    self.version.version.code
                ));
                if let Some(res) = &self.version.version_res {
                    out.push_str(&version_res_line(res, "\t\t"));
                }
                out.push_str("\t\t</firmware_version>\n");
                out.push_str("<partitions>\n");
                out.push_str(&format!(
                    "\t<partitionsNum>{}</partitionsNum>\n",
                    self.partitions_num
                ));
                for p in &self.partitions {
                    out.push_str("\t<partition>\n");
                    out.push_str(&format!("\t\t<type>{}</type>\n", p.ptype.as_str()));
                    out.push_str(&format!("\t\t<name>{}</name>\n", p.name));
                    out.push_str(&format!("\t\t<file_id>{}</file_id>\n", p.file_id));
                    out.push_str(&format!("\t\t<file_name>{}</file_name>\n", p.file_name));
                    if let Some(sid) = p.storage_id {
                        out.push_str(&format!("\t\t<storage_id>{sid}</storage_id>\n"));
                    }
                    out.push_str(&format!("\t\t<file_size>0x{:x}</file_size>\n", p.file_size));
                    out.push_str(&format!("\t\t<checksum>0x{:08x}</checksum>\n", p.checksum));
                    out.push_str("\t</partition>\n");
                }
                out.push_str("\t</partitions>\n");
                out.push_str("\t</ota_firmware>\n");
            }
            XmlStyle::Compact => {
                out.push_str("<ota_firmware>\n");
                out.push_str("<firmware_version>\n");
                out.push_str(&format!(
                    "<version_code>0x{:x}</version_code>\n",
                    self.version.version.code
                ));
                out.push_str(&format!(
                    "<version_name>{}</version_name>\n",
                    self.version.version.name
                ));
                if let Some(res) = &self.version.version_res {
                    out.push_str(&version_res_line(res, ""));
                }
                out.push_str(&format!(
                    "<board_name>{}</board_name>\n",
                    self.version.board_name
                ));
                out.push_str("</firmware_version>\n");
                out.push_str("<partitions>\n");
                out.push_str(&format!(
                    "<partitionsNum>{}</partitionsNum>\n",
                    self.partitions_num
                ));
                for p in &self.partitions {
                    out.push_str("<partition>\n");
                    out.push_str(&format!("<type>{}</type>\n", p.ptype.as_str()));
                    out.push_str(&format!("<name>{}</name>\n", p.name));
                    out.push_str(&format!("<file_id>{}</file_id>\n", p.file_id));
                    out.push_str(&format!("<file_name>{}</file_name>\n", p.file_name));
                    if let Some(sid) = p.storage_id {
                        out.push_str(&format!("<storage_id>{sid}</storage_id>\n"));
                    }
                    out.push_str(&format!("<file_size>0x{:x}</file_size>\n", p.file_size));
                    out.push_str(&format!("<checksum>0x{:08x}</checksum>\n", p.checksum));
                    out.push_str("</partition>\n");
                }
                out.push_str("</partitions>\n");
                out.push_str("</ota_firmware>\n");
            }
        }
        out
    }
}

fn version_res_line(value: &str, indent: &str) -> String {
    if value.is_empty() {
        format!("{indent}<version_res />\n")
    } else {
        format!("{indent}<version_res>{value}</version_res>\n")
    }
}

fn number_u32(text: &str, field: &str) -> std::result::Result<u32, ManifestError> {
    let t = text.trim();
    let parsed = if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
    } else {
        t.parse::<u32>()
    };
    parsed.map_err(|_| ManifestError::BadNumber {
        field: field.to_string(),
        value: t.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Minimal XML reader
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct Node {
    name: String,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    fn text_of(&self, name: &str) -> std::result::Result<&str, ManifestError> {
        self.child(name)
            .map(|n| n.text.as_str())
            .ok_or_else(|| ManifestError::MissingElement(format!("{}/{name}", self.name)))
    }
}

fn parse_document(input: &str) -> std::result::Result<Node, XmlError> {
    let bytes = input.as_bytes();
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let mut pos = 0usize;

    while pos < bytes.len() {
        if bytes[pos] != b'<' {
            // Character data.
            let start = pos;
            while pos < bytes.len() && bytes[pos] != b'<' {
                pos += 1;
            }
            if let Some(top) = stack.last_mut() {
                let chunk = String::from_utf8_lossy(&bytes[start..pos]);
                if !chunk.trim().is_empty() {
                    top.text.push_str(&chunk);
                }
            }
            continue;
        }

        // `<!-- comment -->`
        if input[pos..].starts_with("<!--") {
            let end = input[pos + 4..].find("-->").ok_or(XmlError::Unterminated)?;
            pos = pos + 4 + end + 3;
            continue;
        }
        // `<?declaration?>` and `<!DOCTYPE ...>`
        if input[pos..].starts_with("<?") {
            let end = input[pos + 2..].find("?>").ok_or(XmlError::Unterminated)?;
            pos = pos + 2 + end + 2;
            continue;
        }
        if input[pos..].starts_with("<!") {
            let end = input[pos + 2..].find('>').ok_or(XmlError::Unterminated)?;
            pos = pos + 2 + end + 1;
            continue;
        }

        let close = input[pos..].find('>').ok_or(XmlError::UnexpectedEnd)?;
        let tag_end = pos + close;
        let raw = &input[pos + 1..tag_end];

        if let Some(name) = raw.strip_prefix('/') {
            let name = name.trim();
            let node = stack
                .pop()
                .ok_or_else(|| XmlError::StrayClose(name.to_string()))?;
            if node.name != name {
                return Err(XmlError::MismatchedTag {
                    expected: node.name,
                    found: name.to_string(),
                });
            }
            match stack.last_mut() {
                Some(parent) => parent.children.push(node),
                None => {
                    if root.is_some() {
                        return Err(XmlError::StrayClose(name.to_string()));
                    }
                    root = Some(node);
                }
            }
        } else {
            let self_closing = raw.ends_with('/');
            let body = if self_closing {
                &raw[..raw.len() - 1]
            } else {
                raw
            };
            let name: String = body
                .chars()
                .take_while(|c| !c.is_ascii_whitespace())
                .collect();
            if name.is_empty() {
                pos = tag_end + 1;
                continue;
            }
            let node = Node {
                name: name.clone(),
                children: Vec::new(),
                text: String::new(),
            };
            if self_closing {
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => {
                        if root.is_some() {
                            return Err(XmlError::StrayClose(name));
                        }
                        root = Some(node);
                    }
                }
            } else {
                stack.push(node);
            }
        }

        pos = tag_end + 1;
    }

    if !stack.is_empty() {
        return Err(XmlError::UnexpectedEnd);
    }
    root.ok_or(XmlError::Empty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn parses_every_embedded_manifest() {
        for (board, xml) in fixtures::ALL {
            let m = OtaManifest::parse(xml).unwrap_or_else(|e| panic!("{board}: {e}"));
            assert_eq!(m.board_name(), *board);
            assert!(m.code_derivation_consistent(), "{board}: version code rule");
            m.validate().unwrap();
        }
    }

    #[test]
    fn tabbed_manifests_round_trip_byte_for_byte() {
        for (board, xml) in fixtures::TABBED {
            let m = OtaManifest::parse(xml).unwrap();
            assert_eq!(m.to_xml(XmlStyle::Tabbed), *xml, "{board}");
        }
    }

    #[test]
    fn compact_manifests_round_trip_byte_for_byte() {
        for (board, xml) in fixtures::COMPACT {
            let m = OtaManifest::parse(xml).unwrap();
            assert_eq!(m.to_xml(XmlStyle::Compact), *xml, "{board}");
        }
    }

    #[test]
    fn parses_hex_and_decimal_numbers() {
        let xml = r#"<?xml version='1.0'?>
<ota_firmware>
<firmware_version>
<board_name>b</board_name><version_name>1.0</version_name><version_code>256</version_code>
</firmware_version>
<partitions><partitionsNum>1</partitionsNum>
<partition><type>BOOT</type><name>fw0_boot</name><file_id>1</file_id>
<file_name>x.bin</file_name><file_size>0x1000</file_size><checksum>0x10</checksum></partition>
</partitions></ota_firmware>"#;
        let m = OtaManifest::parse(xml).unwrap();
        assert_eq!(m.version.version.code, 256);
        assert_eq!(m.partitions[0].file_size, 0x1000);
        assert_eq!(m.partitions[0].checksum, 0x10);
        assert_eq!(m.partitions[0].storage_id, None);
    }

    #[test]
    fn rejects_partition_count_mismatch() {
        let xml = fixtures::WGO2_RX_1_93.replace("<partitionsNum>2", "<partitionsNum>3");
        let err = OtaManifest::parse(&xml).unwrap_err();
        assert!(
            matches!(
                err,
                ManifestError::PartitionCountMismatch {
                    declared: 3,
                    actual: 2
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn rejects_unknown_root() {
        let err = OtaManifest::parse("<nope/>").unwrap_err();
        assert_eq!(err, ManifestError::UnexpectedRoot("nope".into()));
    }

    #[test]
    fn rejects_duplicate_file_ids() {
        let xml = fixtures::WGO2_RX_1_93.replace("<file_id>3</file_id>", "<file_id>1</file_id>");
        let err = OtaManifest::parse(&xml).unwrap_err();
        assert_eq!(err, ManifestError::DuplicateFileId(1));
    }

    #[test]
    fn rejects_mismatched_tags() {
        let xml = "<a><b></c></a>";
        let err = OtaManifest::parse(xml).unwrap_err();
        assert!(matches!(
            err,
            ManifestError::Xml(XmlError::MismatchedTag { .. })
        ));
    }

    #[test]
    fn payload_files_are_deduplicated() {
        let m = OtaManifest::parse(fixtures::WM_TX_2_31).unwrap();
        assert_eq!(
            m.payload_files(),
            vec!["app.bin", "tone.bin", "mbrec.bin", "param.bin"]
        );
        assert_eq!(m.total_size(), 0x9ca60 + 0x1b1a0 + 0x1000 + 0x400);
    }
}

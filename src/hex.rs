//! Intel HEX decoding.
//!
//! The bridge contains an Intel HEX decoder with these user-visible failures:
//!
//! * `Intel HEX File decode aborted; ':' missing in first line.`
//! * `Unknown Intel HEX record @ line `
//! * `Invalid hex character`
//! * `Checksum error @ line `
//!
//! (all present verbatim in `rode_bridge.dll`'s `.rdata`). This module
//! implements the standard record set that follows from those messages:
//! `00` data, `01` end-of-file, `02` extended segment address, `03` start
//! segment address, `04` extended linear address and `05` start linear address.

use crate::crc32;
use std::fmt;

/// Intel HEX decode failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HexError {
    /// The first non-empty line did not start with `:`.
    MissingColonOnFirstLine,
    /// A record used an unknown record type.
    UnknownRecord {
        /// 1-based line number.
        line: usize,
        /// The record type byte.
        record_type: u8,
    },
    /// A record contained a non-hexadecimal character.
    InvalidHexChar {
        /// 1-based line number.
        line: usize,
        /// The offending byte.
        byte: u8,
    },
    /// A record's byte-count field disagrees with its payload length.
    LengthMismatch {
        /// 1-based line number.
        line: usize,
        /// Byte count from the record header.
        declared: usize,
        /// Bytes actually present.
        actual: usize,
    },
    /// The record checksum did not sum to zero.
    Checksum {
        /// 1-based line number.
        line: usize,
        /// Checksum byte as written.
        stored: u8,
        /// Checksum byte required for a valid record.
        required: u8,
    },
    /// An extended-address record had the wrong payload length.
    BadAddressRecord {
        /// 1-based line number.
        line: usize,
    },
    /// A data record would place bytes beyond the 32-bit address space.
    AddressOverflow {
        /// 1-based line number.
        line: usize,
    },
}

impl fmt::Display for HexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HexError::MissingColonOnFirstLine => write!(
                f,
                "Intel HEX File decode aborted; ':' missing in first line."
            ),
            HexError::UnknownRecord { line, record_type } => {
                write!(
                    f,
                    "Unknown Intel HEX record @ line {line} (type {record_type:02x})"
                )
            }
            HexError::InvalidHexChar { line, byte } => {
                write!(f, "Invalid hex character @ line {line} (0x{byte:02x})")
            }
            HexError::LengthMismatch {
                line,
                declared,
                actual,
            } => write!(
                f,
                "Intel HEX record length mismatch @ line {line}: declared {declared}, got {actual}"
            ),
            HexError::Checksum {
                line,
                stored,
                required,
            } => write!(
                f,
                "Checksum error @ line {line} (stored 0x{stored:02x}, requires 0x{required:02x})"
            ),
            HexError::BadAddressRecord { line } => {
                write!(f, "Intel HEX address record malformed @ line {line}")
            }
            HexError::AddressOverflow { line } => {
                write!(
                    f,
                    "Intel HEX data record exceeds 32-bit address space @ line {line}"
                )
            }
        }
    }
}

impl std::error::Error for HexError {}

/// One decoded Intel HEX record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HexRecord {
    /// `00` — payload bytes at `address` within the current base.
    Data {
        /// Offset within the current segment/base.
        address: u16,
        /// Payload.
        data: Vec<u8>,
    },
    /// `01` — end of file.
    EndOfFile,
    /// `02` — base = `value << 4`.
    ExtendedSegmentAddress(u16),
    /// `03` — start segment address (`CS:IP`).
    StartSegmentAddress {
        /// Code segment.
        cs: u16,
        /// Instruction pointer.
        ip: u16,
    },
    /// `04` — base = `value << 16`.
    ExtendedLinearAddress(u16),
    /// `05` — initial instruction pointer.
    StartLinearAddress(u32),
}

impl HexRecord {
    /// Decodes a single record line (leading `:` required).
    pub fn parse(line: &str, line_no: usize) -> std::result::Result<HexRecord, HexError> {
        let s = line.trim();
        let s = s.strip_prefix(':').ok_or(if line_no == 1 {
            HexError::MissingColonOnFirstLine
        } else {
            HexError::InvalidHexChar {
                line: line_no,
                byte: b':',
            }
        })?;

        let bytes = decode_hex(s, line_no)?;
        if bytes.len() < 5 {
            return Err(HexError::LengthMismatch {
                line: line_no,
                declared: bytes.first().copied().unwrap_or(0) as usize,
                actual: bytes.len().saturating_sub(5),
            });
        }

        let declared = bytes[0] as usize;
        let actual = bytes.len() - 5;
        if declared != actual {
            return Err(HexError::LengthMismatch {
                line: line_no,
                declared,
                actual,
            });
        }

        // Every byte, including the stored checksum, must sum to 0 mod 256.
        let sum = bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b));
        if sum != 0 {
            let stored = *bytes.last().unwrap();
            return Err(HexError::Checksum {
                line: line_no,
                stored,
                required: stored.wrapping_sub(sum),
            });
        }

        let address = u16::from_be_bytes([bytes[1], bytes[2]]);
        let record_type = bytes[3];
        let payload = &bytes[4..4 + declared];

        match record_type {
            0x00 => Ok(HexRecord::Data {
                address,
                data: payload.to_vec(),
            }),
            0x01 => Ok(HexRecord::EndOfFile),
            0x02 => {
                if payload.len() != 2 {
                    return Err(HexError::BadAddressRecord { line: line_no });
                }
                Ok(HexRecord::ExtendedSegmentAddress(u16::from_be_bytes([
                    payload[0], payload[1],
                ])))
            }
            0x03 => {
                if payload.len() != 4 {
                    return Err(HexError::BadAddressRecord { line: line_no });
                }
                Ok(HexRecord::StartSegmentAddress {
                    cs: u16::from_be_bytes([payload[0], payload[1]]),
                    ip: u16::from_be_bytes([payload[2], payload[3]]),
                })
            }
            0x04 => {
                if payload.len() != 2 {
                    return Err(HexError::BadAddressRecord { line: line_no });
                }
                Ok(HexRecord::ExtendedLinearAddress(u16::from_be_bytes([
                    payload[0], payload[1],
                ])))
            }
            0x05 => {
                if payload.len() != 4 {
                    return Err(HexError::BadAddressRecord { line: line_no });
                }
                Ok(HexRecord::StartLinearAddress(u32::from_be_bytes([
                    payload[0], payload[1], payload[2], payload[3],
                ])))
            }
            other => Err(HexError::UnknownRecord {
                line: line_no,
                record_type: other,
            }),
        }
    }
}

fn decode_hex(s: &str, line_no: usize) -> std::result::Result<Vec<u8>, HexError> {
    let bytes = s.as_bytes();
    if bytes.len() % 2 != 0 {
        return Err(HexError::LengthMismatch {
            line: line_no,
            declared: 0,
            actual: bytes.len() / 2,
        });
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_nibble(bytes[i]).ok_or(HexError::InvalidHexChar {
            line: line_no,
            byte: bytes[i],
        })?;
        let lo = hex_nibble(bytes[i + 1]).ok_or(HexError::InvalidHexChar {
            line: line_no,
            byte: bytes[i + 1],
        })?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

const fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// A contiguous run of bytes at a 32-bit address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Start address.
    pub address: u32,
    /// Payload bytes.
    pub data: Vec<u8>,
}

impl Segment {
    /// One-past-the-end address.
    pub fn end(&self) -> u64 {
        self.address as u64 + self.data.len() as u64
    }
}

/// A decoded Intel HEX image: address-tagged segments, sorted and merged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HexImage {
    /// Segments in ascending address order.
    pub segments: Vec<Segment>,
    /// Entry point from a `03`/`05` record, when present.
    pub entry_point: Option<u32>,
    /// Whether a `01` end-of-file record was seen.
    pub saw_eof: bool,
}

impl HexImage {
    /// Decodes a whole Intel HEX document.
    pub fn parse(text: &str) -> std::result::Result<HexImage, HexError> {
        let records = Self::parse_records(text)?;
        Ok(Self::from_records(&records))
    }

    /// Decodes every record, preserving order.
    pub fn parse_records(text: &str) -> std::result::Result<Vec<HexRecord>, HexError> {
        let mut out = Vec::new();
        let mut saw_any = false;
        for (idx, raw) in text.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw.trim_end_matches(['\r', '\n']).trim();
            if line.is_empty() {
                continue;
            }
            if !saw_any && !line.starts_with(':') {
                return Err(HexError::MissingColonOnFirstLine);
            }
            saw_any = true;
            out.push(HexRecord::parse(line, line_no)?);
        }
        if out.is_empty() {
            return Err(HexError::MissingColonOnFirstLine);
        }
        Ok(out)
    }

    /// Folds records into address-tagged segments.
    pub fn from_records(records: &[HexRecord]) -> HexImage {
        let mut base: u64 = 0;
        let mut segments: Vec<Segment> = Vec::new();
        let mut entry_point = None;
        let mut saw_eof = false;

        for record in records {
            match record {
                HexRecord::Data { address, data } => {
                    let start = base + *address as u64;
                    segments.push(Segment {
                        address: start as u32,
                        data: data.clone(),
                    });
                }
                HexRecord::EndOfFile => {
                    saw_eof = true;
                    break;
                }
                HexRecord::ExtendedSegmentAddress(value) => {
                    base = (*value as u64) << 4;
                }
                HexRecord::ExtendedLinearAddress(value) => {
                    base = (*value as u64) << 16;
                }
                HexRecord::StartSegmentAddress { cs, ip } => {
                    entry_point = Some(((*cs as u32) << 4).wrapping_add(*ip as u32));
                }
                HexRecord::StartLinearAddress(value) => {
                    entry_point = Some(*value);
                }
            }
        }

        merge_segments(&mut segments);
        segments.sort_by_key(|seg| seg.address);

        HexImage {
            segments,
            entry_point,
            saw_eof,
        }
    }

    /// Total payload bytes across all segments.
    pub fn total_bytes(&self) -> usize {
        self.segments.iter().map(|s| s.data.len()).sum()
    }

    /// Lowest address in the image.
    pub fn min_address(&self) -> Option<u32> {
        self.segments.first().map(|s| s.address)
    }

    /// Highest address touched by the image.
    pub fn max_address(&self) -> Option<u64> {
        self.segments.iter().map(Segment::end).max()
    }

    /// CRC-32 over the concatenated segment payloads in address order.
    pub fn crc32(&self) -> u32 {
        let mut hasher = crc32::Crc32::new();
        for seg in &self.segments {
            hasher.update(&seg.data);
        }
        hasher.finish()
    }

    /// Render as raw bytes covering `[min, max)`; holes are filled with
    /// `fill`.
    pub fn to_binary(&self, fill: u8) -> Vec<u8> {
        let min = match self.min_address() {
            Some(a) => a as usize,
            None => return Vec::new(),
        };
        let max = self.max_address().unwrap_or(min as u64) as usize;
        let mut out = vec![fill; max.saturating_sub(min)];
        for seg in &self.segments {
            let off = seg.address as usize - min;
            out[off..off + seg.data.len()].copy_from_slice(&seg.data);
        }
        out
    }
}

fn merge_segments(segments: &mut Vec<Segment>) {
    segments.sort_by_key(|s| s.address);
    let mut merged: Vec<Segment> = Vec::with_capacity(segments.len());
    for seg in segments.drain(..) {
        match merged.last_mut() {
            Some(last) if last.end() == seg.address as u64 => {
                last.data.extend_from_slice(&seg.data)
            }
            Some(last) if (seg.address as u64) < last.end() => {
                // Overlapping records: later bytes win.
                let off = (seg.address as u64 - last.address as u64) as usize;
                let end = off + seg.data.len();
                if end > last.data.len() {
                    last.data.resize(end, 0);
                }
                last.data[off..end].copy_from_slice(&seg.data);
            }
            _ => merged.push(seg),
        }
    }
    *segments = merged;
}

/// Encodes `data` as Intel HEX starting at `address`, 16 bytes per record.
pub fn encode(data: &[u8], address: u32) -> String {
    let mut out = String::new();
    let mut cursor = address;
    let mut bank = address >> 16;

    if bank != 0 {
        push_record(&mut out, 0, 0x04, &[(bank >> 8) as u8, (bank & 0xff) as u8]);
    }

    for chunk in data.chunks(16) {
        // The low 16 bits of the address go in the record header; the upper
        // 16 bits come from an extended linear address record.
        let this_bank = cursor >> 16;
        if this_bank != bank {
            bank = this_bank;
            push_record(&mut out, 0, 0x04, &[(bank >> 8) as u8, (bank & 0xff) as u8]);
        }
        push_record(&mut out, (cursor & 0xffff) as u16, 0x00, chunk);
        cursor = cursor.wrapping_add(chunk.len() as u32);
    }

    push_record(&mut out, 0, 0x01, &[]);
    out
}

fn push_record(out: &mut String, address: u16, record_type: u8, payload: &[u8]) {
    let mut sum = 0u8;
    let mut line = String::new();
    line.push(':');
    let header = [
        payload.len() as u8,
        (address >> 8) as u8,
        (address & 0xff) as u8,
        record_type,
    ];
    for b in header {
        sum = sum.wrapping_add(b);
        line.push_str(&format!("{b:02X}"));
    }
    for &b in payload {
        sum = sum.wrapping_add(b);
        line.push_str(&format!("{b:02X}"));
    }
    line.push_str(&format!("{:02X}", 0u8.wrapping_sub(sum)));
    out.push_str(&line);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    // A small, valid Intel HEX file: two data records + EOF.
    const SAMPLE: &str = ":020000040800F2\n\
                          :10000000000102030405060708090A0B0C0D0E0F78\n\
                          :0400100010111213A6\n\
                          :00000001FF\n";

    #[test]
    fn parses_sample() {
        let image = HexImage::parse(SAMPLE).unwrap();
        assert!(image.saw_eof);
        assert_eq!(image.total_bytes(), 20);
        assert_eq!(image.min_address(), Some(0x0800_0000));
        assert_eq!(image.max_address(), Some(0x0800_0014));
        assert_eq!(&image.segments[0].data[..4], &[0, 1, 2, 3]);
    }

    #[test]
    fn detects_missing_colon() {
        assert_eq!(
            HexImage::parse("10000000000102030405060708090A0B0C0D0E0F78\n"),
            Err(HexError::MissingColonOnFirstLine)
        );
    }

    #[test]
    fn detects_bad_checksum() {
        let err = HexImage::parse(":10000000000102030405060708090A0B0C0D0E0F79\n").unwrap_err();
        assert!(matches!(err, HexError::Checksum { line: 1, .. }), "{err}");
    }

    #[test]
    fn detects_unknown_record() {
        // type 06 is not defined
        let mut sum = 0u8;
        for b in [0x00u8, 0x00, 0x00, 0x06] {
            sum = sum.wrapping_add(b);
        }
        let line = format!(":00000006{:02X}", 0u8.wrapping_sub(sum));
        let err = HexImage::parse(&line).unwrap_err();
        assert_eq!(
            err,
            HexError::UnknownRecord {
                line: 1,
                record_type: 6
            }
        );
    }

    #[test]
    fn detects_invalid_hex_char() {
        let err = HexImage::parse(":02000004080ZF2\n").unwrap_err();
        assert!(matches!(err, HexError::InvalidHexChar { .. }), "{err}");
    }

    #[test]
    fn round_trips_encode() {
        let data: Vec<u8> = (0..=255u8).collect();
        let text = encode(&data, 0x0800_0000);
        let image = HexImage::parse(&text).unwrap();
        assert_eq!(image.to_binary(0xff), data);
    }
}

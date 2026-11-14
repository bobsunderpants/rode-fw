//! Firmware versions.
//!
//! Every OTA manifest carries a human-readable `<version_name>` and a numeric
//! `<version_code>`. Across all twelve manifests embedded in
//! `rode_bridge.dll` the code is the decimal digits of the name read as a
//! hexadecimal integer:
//!
//! | `version_name` | digits | `version_code` |
//! |----------------|--------|----------------|
//! | `1.93`         | `193`  | `0x193`        |
//! | `2.50`         | `250`  | `0x250`        |
//! | `1.47`         | `147`  | `0x147`        |
//! | `2.31`         | `231`  | `0x231`        |
//!
//! [`FwVersion::code_from_name`] reproduces that rule and
//! [`FwVersion::declared_code_matches`] lets callers detect manifests that
//! deviate from it.

use std::cmp::Ordering;
use std::fmt;

/// Version related failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    /// The name contained no decimal digits.
    NoDigits(String),
    /// The digit string does not fit in a `u32` when read as hex.
    CodeOverflow(String),
    /// A numeric segment could not be parsed.
    BadSegment(String),
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionError::NoDigits(name) => {
                write!(f, "version name {name:?} contains no decimal digits")
            }
            VersionError::CodeOverflow(name) => {
                write!(
                    f,
                    "version name {name:?} has too many digits for a 32-bit code"
                )
            }
            VersionError::BadSegment(seg) => write!(f, "invalid version segment {seg:?}"),
        }
    }
}

impl std::error::Error for VersionError {}

/// A firmware version: display name plus numeric code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FwVersion {
    /// `version_name` as shipped, e.g. `"1.93"`.
    pub name: String,
    /// `version_code` as shipped, e.g. `0x193`.
    pub code: u32,
}

impl FwVersion {
    /// Builds a version from its name, deriving `code` with the documented
    /// digit-as-hex rule.
    pub fn from_name(name: &str) -> std::result::Result<Self, VersionError> {
        let code = Self::code_from_name(name)?;
        Ok(FwVersion {
            name: name.trim().to_string(),
            code,
        })
    }

    /// Builds a version from name and declared code without checking that the
    /// two agree (the manifest parser stores whatever the file says).
    pub fn new(name: &str, code: u32) -> Self {
        FwVersion {
            name: name.trim().to_string(),
            code,
        }
    }

    /// The digits of `name`, read as a hexadecimal integer.
    pub fn code_from_name(name: &str) -> std::result::Result<u32, VersionError> {
        let digits: String = name.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return Err(VersionError::NoDigits(name.to_string()));
        }
        if digits.len() > 8 {
            return Err(VersionError::CodeOverflow(name.to_string()));
        }
        u32::from_str_radix(&digits, 16).map_err(|_| VersionError::CodeOverflow(name.to_string()))
    }

    /// True when the shipped `code` equals the code derived from `name`.
    pub fn declared_code_matches(&self) -> bool {
        Self::code_from_name(&self.name)
            .map(|derived| derived == self.code)
            .unwrap_or(false)
    }

    /// Numeric segments, e.g. `1.93` -> `[1, 93]`.
    pub fn segments(&self) -> std::result::Result<Vec<u32>, VersionError> {
        let cleaned = self.name.trim().trim_start_matches(['v', 'V']);
        let mut out = Vec::new();
        for seg in cleaned.split(['.', '-', '_', '+']) {
            if seg.is_empty() {
                continue;
            }
            let digits: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                break;
            }
            out.push(
                digits
                    .parse::<u32>()
                    .map_err(|_| VersionError::BadSegment(seg.to_string()))?,
            );
        }
        if out.is_empty() {
            return Err(VersionError::BadSegment(self.name.clone()));
        }
        Ok(out)
    }

    /// Compares two versions numerically, segment by segment.
    ///
    /// ```
    /// use rode_fw::FwVersion;
    /// let a = FwVersion::from_name("1.93").unwrap();
    /// let b = FwVersion::from_name("2.50").unwrap();
    /// assert!(a < b);
    /// ```
    pub fn compare(&self, other: &FwVersion) -> Ordering {
        match (self.segments(), other.segments()) {
            (Ok(a), Ok(b)) => {
                for (x, y) in a.iter().zip(b.iter()) {
                    match x.cmp(y) {
                        Ordering::Equal => continue,
                        non_eq => return non_eq,
                    }
                }
                a.len().cmp(&b.len())
            }
            // A name we cannot parse falls back to the numeric code.
            _ => self.code.cmp(&other.code),
        }
    }

    /// True when `self` is strictly newer than `other`.
    pub fn is_newer_than(&self, other: &FwVersion) -> bool {
        self.compare(other) == Ordering::Greater
    }
}

impl PartialOrd for FwVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FwVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

impl fmt::Display for FwVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (0x{:x})", self.name, self.code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_follow_digit_as_hex_rule() {
        for (name, code) in [
            ("1.93", 0x193u32),
            ("2.50", 0x250),
            ("1.47", 0x147),
            ("2.31", 0x231),
            ("1.0.6", 0x106),
        ] {
            assert_eq!(FwVersion::code_from_name(name).unwrap(), code, "{name}");
            let v = FwVersion::from_name(name).unwrap();
            assert!(v.declared_code_matches());
        }
    }

    #[test]
    fn ordering_is_numeric_not_lexical() {
        let v = |s: &str| FwVersion::from_name(s).unwrap();
        assert!(v("1.9") < v("1.10"));
        assert!(v("1.93") < v("2.50"));
        assert!(v("2.31") < v("2.50"));
        assert_eq!(v("2.50").compare(&v("2.50")), Ordering::Equal);
        assert!(v("2.50").is_newer_than(&v("1.93")));
    }

    #[test]
    fn segments_ignore_prefixes_and_suffixes() {
        let v = FwVersion::from_name("v1.2.3-beta").unwrap();
        assert_eq!(v.segments().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn rejects_names_without_digits() {
        assert!(FwVersion::from_name("release").is_err());
    }
}

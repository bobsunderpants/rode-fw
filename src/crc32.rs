//! CRC-32 (IEEE 802.3, reflected, poly `0xEDB88320`).
//!
//! The bridge links a textbook reflected CRC-32 table: the bytes at
//! `0x181d85580` decode to
//! `0x59b33d17 0x2eb40d81 0xb7bd5c3b 0xc0ba6cad 0xedb88320 ...`, i.e. table
//! entries `0x7c..=0x81` of the standard reflected table (ascending index).
//! `<checksum>` in the OTA manifest is a 32-bit value per partition, and the
//! same payload (`tone.bin`, `mbrec.bin`) carries the same checksum across
//! different boards, so the value is content-derived. CRC-32/IEEE is the
//! algorithm the binary demonstrably contains.

/// Standard CRC-32 lookup table.
pub const TABLE: [u32; 256] = build_table();

const fn build_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// CRC-32 of `data`, i.e. `crc32("123456789") == 0xcbf43926`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut hasher = Crc32::new();
    hasher.update(data);
    hasher.finish()
}

/// Streaming CRC-32 accumulator.
#[derive(Debug, Clone)]
pub struct Crc32 {
    state: u32,
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

impl Crc32 {
    /// Start a new accumulator (init value `0xFFFF_FFFF`).
    pub const fn new() -> Self {
        Self { state: 0xffff_ffff }
    }

    /// Feed more bytes.
    pub fn update(&mut self, data: &[u8]) {
        let mut crc = self.state;
        for &b in data {
            let idx = ((crc ^ b as u32) & 0xff) as usize;
            crc = (crc >> 8) ^ TABLE[idx];
        }
        self.state = crc;
    }

    /// Final value (state XOR `0xFFFF_FFFF`).
    pub const fn finish(&self) -> u32 {
        self.state ^ 0xffff_ffff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414f_a339
        );
    }

    #[test]
    fn table_matches_dll_bytes() {
        // The DLL's table at 0x181d85580 is the standard reflected CRC-32
        // table; the observed words are entries 0x7c..=0x81.
        assert_eq!(TABLE[0x7c], 0x59b3_3d17);
        assert_eq!(TABLE[0x7d], 0x2eb4_0d81);
        assert_eq!(TABLE[0x7e], 0xb7bd_5c3b);
        assert_eq!(TABLE[0x7f], 0xc0ba_6cad);
        assert_eq!(TABLE[0x80], 0xedb8_8320);
        assert_eq!(TABLE[0x81], 0x9abf_b3b6);
    }

    #[test]
    fn streaming_matches_one_shot() {
        let data: Vec<u8> = (0..1000u32).map(|i| (i * 7 + 3) as u8).collect();
        let mut s = Crc32::new();
        for chunk in data.chunks(37) {
            s.update(chunk);
        }
        assert_eq!(s.finish(), crc32(&data));
    }
}

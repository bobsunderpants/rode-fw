//! MD5, used by the manifest downloader (`firmware-md5`, `archive.md5`,
//! `os-md5`).
//!
//! The DLL imports no crypto library, but MD5's round constant `0xd76aa478`
//! (and the rest of the table) is present in `.text`/`.rdata`, so the digest is
//! implemented in-tree. This module is a straightforward RFC 1321
//! implementation; it exists to reproduce the verification the app performs,
//! not as a secure primitive.

/// Number of bytes in an MD5 digest.
pub const DIGEST_LEN: usize = 16;

const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

const K: [u32; 64] = [
    0xd76a_a478,
    0xe8c7_b756,
    0x2420_70db,
    0xc1bd_ceee,
    0xf57c_0faf,
    0x4787_c62a,
    0xa830_4613,
    0xfd46_9501,
    0x6980_98d8,
    0x8b44_f7af,
    0xffff_5bb1,
    0x895c_d7be,
    0x6b90_1122,
    0xfd98_7193,
    0xa679_438e,
    0x49b4_0821,
    0xf61e_2562,
    0xc040_b340,
    0x265e_5a51,
    0xe9b6_c7aa,
    0xd62f_105d,
    0x0244_1453,
    0xd8a1_e681,
    0xe7d3_fbc8,
    0x21e1_cde6,
    0xc337_07d6,
    0xf4d5_0d87,
    0x455a_14ed,
    0xa9e3_e905,
    0xfcef_a3f8,
    0x676f_02d9,
    0x8d2a_4c8a,
    0xfffa_3942,
    0x8771_f681,
    0x6d9d_6122,
    0xfde5_380c,
    0xa4be_ea44,
    0x4bde_cfa9,
    0xf6bb_4b60,
    0xbebf_bc70,
    0x289b_7ec6,
    0xeaa1_27fa,
    0xd4ef_3085,
    0x0488_1d05,
    0xd9d4_d039,
    0xe6db_99e5,
    0x1fa2_7cf8,
    0xc4ac_5665,
    0xf429_2244,
    0x432a_ff97,
    0xab94_23a7,
    0xfc93_a039,
    0x655b_59c3,
    0x8f0c_cc92,
    0xffef_f47d,
    0x8584_5dd1,
    0x6fa8_7e4f,
    0xfe2c_e6e0,
    0xa301_4314,
    0x4e08_11a1,
    0xf753_7e82,
    0xbd3a_f235,
    0x2ad7_d2bb,
    0xeb86_d391,
];

/// Computes the 16-byte MD5 digest of `data`.
pub fn md5(data: &[u8]) -> [u8; DIGEST_LEN] {
    let mut state: [u32; 4] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut tail = Vec::with_capacity(72);
    let rem = data.len() % 64;
    let pad = if rem < 56 { 56 - rem } else { 120 - rem };
    tail.extend_from_slice(&data[data.len() - rem..]);
    tail.push(0x80);
    tail.resize(tail.len() + pad - 1, 0);
    tail.extend_from_slice(&bit_len.to_le_bytes());

    let body_len = data.len() - rem;
    let mut chunk = [0u8; 64];
    for block in 0..(body_len + tail.len()) / 64 {
        let src: &[u8] = if block * 64 < body_len {
            &data[block * 64..block * 64 + 64]
        } else {
            let off = block * 64 - body_len;
            chunk.copy_from_slice(&tail[off..off + 64]);
            &chunk
        };
        transform(&mut state, src);
    }

    let mut out = [0u8; DIGEST_LEN];
    for (i, word) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}

fn transform(state: &mut [u32; 4], block: &[u8]) {
    let mut m = [0u32; 16];
    for i in 0..16 {
        m[i] = u32::from_le_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }

    let (mut a, mut b, mut c, mut d) = (state[0], state[1], state[2], state[3]);

    for i in 0..64 {
        let (f, g) = match i / 16 {
            0 => ((b & c) | (!b & d), i),
            1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            2 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let tmp = d;
        d = c;
        c = b;
        let sum = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]);
        b = b.wrapping_add(sum.rotate_left(S[i]));
        a = tmp;
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
}

/// Lower-case hex encoding of an MD5 digest.
pub fn hex_digest(data: &[u8]) -> String {
    let digest = md5(data);
    let mut s = String::with_capacity(32);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Compares an expected hex digest (case-insensitive) with `data`.
pub fn verify_hex(expected: &str, data: &[u8]) -> bool {
    let expected = expected.trim();
    if expected.len() != 32 {
        return false;
    }
    hex_digest(data).eq_ignore_ascii_case(expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc1321_vectors() {
        assert_eq!(hex_digest(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex_digest(b"a"), "0cc175b9c0f1b6a831c399e269772661");
        assert_eq!(hex_digest(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            hex_digest(b"message digest"),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
        assert_eq!(
            hex_digest(b"abcdefghijklmnopqrstuvwxyz"),
            "c3fcd3d76192e4007dfb496cca67e13b"
        );
        assert_eq!(
            hex_digest(
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
            ),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
    }

    #[test]
    fn block_boundaries() {
        // 55, 56, 57 and 64 byte inputs exercise the padding edge cases.
        for n in [55usize, 56, 57, 63, 64, 65, 119, 120, 127, 128] {
            let data = vec![0x5a; n];
            assert_eq!(md5(&data).len(), 16, "len {n}");
        }
        // Known digests around the padding edge.
        assert_eq!(
            hex_digest(&[0x61u8; 56]),
            "3b0c8ac703f828b04c6c197006d17218"
        );
        assert_eq!(hex_digest(&[0u8; 64]), "3b5d3c7d207e37dceeedd301e35e2e58");
    }

    #[test]
    fn verify_helper() {
        assert!(verify_hex("900150983CD24FB0D6963F7D28E17F72", b"abc"));
        assert!(!verify_hex("900150983cd24fb0d6963f7d28e17f73", b"abc"));
        assert!(!verify_hex("abc", b"abc"));
    }
}

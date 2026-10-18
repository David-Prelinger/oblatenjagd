//! FNV-1a (64 bit), used by `Game::state_hash`.

/// An FNV-1a 64-bit hasher. Values are fed explicitly, little-endian.
#[derive(Clone, Copy, Debug)]
pub struct Fnv(u64);

impl Fnv {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    /// A fresh hasher.
    pub const fn new() -> Fnv {
        Fnv(Fnv::OFFSET)
    }

    /// Feeds raw bytes.
    pub fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(Fnv::PRIME);
        }
    }

    /// Feeds a `u8`.
    pub fn u8(&mut self, v: u8) {
        self.bytes(&[v]);
    }

    /// Feeds a `bool` as one byte.
    pub fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }

    /// Feeds a `u16`.
    pub fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes());
    }

    /// Feeds a `u32`.
    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }

    /// Feeds an `i32`.
    pub fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }

    /// Feeds a `u64`.
    pub fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }

    /// The hash so far.
    pub const fn finish(&self) -> u64 {
        self.0
    }
}

impl Default for Fnv {
    fn default() -> Fnv {
        Fnv::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        assert_eq!(Fnv::new().finish(), 0xcbf2_9ce4_8422_2325);
        let mut h = Fnv::new();
        h.bytes(b"a");
        assert_eq!(h.finish(), 0xaf63_dc4c_8601_ec8c);
        let mut h = Fnv::new();
        h.bytes(b"foobar");
        assert_eq!(h.finish(), 0x8594_4171_f739_67e8);
    }
}

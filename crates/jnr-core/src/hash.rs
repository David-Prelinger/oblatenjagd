//! FNV-1a (64 bit), used by `Game::state_hash`, and a word-at-a-time
//! hasher for `Game::world_hash`.

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

/// A word-at-a-time 64-bit hasher: one multiply and one xor-shift per
/// 64-bit word (each step is a bijection of the state, so two inputs that
/// differ in one word always give different states after it). Several
/// times faster than byte-wise FNV-1a for long runs of small values, for
/// hashes that are only compared within one run (`Game::world_hash`, the
/// solver's de-duplication key, E35); `state_hash` keeps FNV-1a.
#[derive(Clone, Copy, Debug)]
pub struct WordHash(u64);

impl WordHash {
    const SEED: u64 = 0x243f_6a88_85a3_08d3;
    const K: u64 = 0x9e37_79b9_7f4a_7c15;

    /// A fresh hasher.
    pub const fn new() -> WordHash {
        WordHash(WordHash::SEED)
    }

    /// Feeds one word.
    pub fn word(&mut self, v: u64) {
        let x = (self.0 ^ v).wrapping_mul(WordHash::K);
        self.0 = x ^ (x >> 31);
    }

    /// The hash so far.
    pub const fn finish(&self) -> u64 {
        self.0
    }
}

impl Default for WordHash {
    fn default() -> WordHash {
        WordHash::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_hash_tells_words_and_their_order_apart() {
        let of = |words: &[u64]| {
            let mut h = WordHash::new();
            for &w in words {
                h.word(w);
            }
            h.finish()
        };
        assert_eq!(of(&[1, 2, 3]), of(&[1, 2, 3]));
        assert_ne!(of(&[1, 2, 3]), of(&[1, 3, 2]));
        assert_ne!(of(&[1, 2]), of(&[1, 2, 0]));
        assert_ne!(of(&[0]), of(&[]));
        // A change in a high bit reaches the low bits of the result.
        assert_ne!(of(&[1 << 63, 7]) & 0xffff, of(&[0, 7]) & 0xffff);
    }

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

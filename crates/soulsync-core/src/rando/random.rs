//! The random numbers of the original randomizer: Java's `java.util.Random`.
//!
//! Every draw has to come out as it does in Java, or the same seed would
//! lead to a different game.

const MULTIPLIER: u64 = 0x5_DEEC_E66D;
const MASK: u64 = (1 << 48) - 1;

#[derive(Debug, Clone)]
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        JavaRandom {
            seed: (seed as u64 ^ MULTIPLIER) & MASK,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(0xB) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    /// Any `int`.
    pub fn int(&mut self) -> i32 {
        self.next(32)
    }

    /// A number from 0 up to, not including, `bound`, which is above 0.
    pub fn below(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "the bound of a random number is positive");
        let mut bits = self.next(31);
        let last = bound - 1;
        if bound & last == 0 {
            return ((i64::from(bound) * i64::from(bits)) >> 31) as i32;
        }
        loop {
            let value = bits % bound;
            // Draws from the uneven top of the range are thrown away.
            if bits.wrapping_sub(value).wrapping_add(last) >= 0 {
                return value;
            }
            bits = self.next(31);
        }
    }

    /// An index into something of `len` entries.
    pub fn index(&mut self, len: usize) -> usize {
        self.below(len as i32) as usize
    }

    pub fn long(&mut self) -> i64 {
        (i64::from(self.next(32)) << 32).wrapping_add(i64::from(self.next(32)))
    }

    pub fn boolean(&mut self) -> bool {
        self.next(1) != 0
    }

    /// A number from 0 up to, not including, 1.
    pub fn double(&mut self) -> f64 {
        let high = i64::from(self.next(26)) << 27;
        (high + i64::from(self.next(27))) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }

    pub fn bytes(&mut self, out: &mut [u8]) {
        for chunk in out.chunks_mut(4) {
            let word = self.next(32).to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
    }

    /// Shuffles like Java's `Collections.shuffle`.
    pub fn shuffle<T>(&mut self, list: &mut [T]) {
        for i in (2..=list.len()).rev() {
            let other = self.index(i);
            list.swap(i - 1, other);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The values Java 21 draws in this order from `new Random(1)`.
    #[test]
    fn draws_what_java_draws() {
        let mut r = JavaRandom::new(1);
        let bounds = [649, 1, 16, 100, 1 << 30, (1 << 30) + 1, 3];
        let ints: Vec<i32> = bounds.iter().map(|&b| r.below(b)).collect();
        assert_eq!(ints, [246, 0, 6, 13, 223_032_127, 77_814_904, 2]);
        assert_eq!(r.int(), -1_465_154_083);
        assert_eq!(r.long(), -594_798_593_157_429_144);
        assert_eq!((r.boolean(), r.boolean()), (false, false));
        assert_eq!(r.double().to_bits(), 4_606_855_500_674_814_171);
        assert_eq!(r.float().to_bits(), 1_064_344_324);
        // Java draws three Gaussian numbers here, which the randomizer never
        // does: two pairs of doubles, both pairs kept.
        for _ in 0..4 {
            r.double();
        }
        let mut bytes = [0u8; 7];
        r.bytes(&mut bytes);
        assert_eq!(bytes.map(|b| b as i8), [54, 122, -54, 40, 100, 82, 71]);
        let mut list: Vec<u8> = (0..10).collect();
        r.shuffle(&mut list);
        assert_eq!(list, [1, 3, 4, 8, 0, 2, 5, 6, 7, 9]);
    }

    #[test]
    fn takes_a_negative_seed() {
        let mut r = JavaRandom::new(-123_456_789_012_345);
        assert_eq!((r.below(1000), r.int()), (724, 95_777_618));
    }
}

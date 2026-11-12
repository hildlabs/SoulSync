//! Settings files of Universal Pokémon Randomizer ZX (`.rnqs`).
//!
//! A file is the randomizer's version (`u32`, big-endian), the length of the
//! settings string and the string itself: Base64 of 51 bytes of options, the
//! ROM name, a CRC-32 of both and a checksum of the custom names file. The
//! randomizer refuses a string whose CRC-32 does not match, so changing an
//! option means writing it anew.
//!
//! SoulSync changes only the options it offers and keeps everything else as
//! the player set it in the randomizer. The byte layout follows
//! `Settings.toString` of the randomizer (GPL-3.0, as SoulSync).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Option bytes before the ROM name.
const OPTIONS: usize = 51;
/// TM moves and TM/HM compatibility.
const TMS: usize = 18;
/// Full HM compatibility, TMs and tutors follow evolutions.
const TMS2: usize = 19;
/// Move tutor moves and compatibility.
const TUTORS: usize = 21;

/// Why a settings file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetError {
    /// Not a settings file of the randomizer.
    Malformed,
    /// Its checksum does not match: the randomizer would refuse it too.
    Checksum,
}

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PresetError::Malformed => write!(f, "not a randomizer settings file (.rnqs)"),
            PresetError::Checksum => write!(f, "the settings file is damaged"),
        }
    }
}

impl std::error::Error for PresetError {}

/// Which Pokémon can learn which TM, or which move tutor's move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compat {
    /// As in the original game.
    Unchanged,
    /// At random, more likely for moves of the Pokémon's own type.
    RandomPreferType,
    /// At random.
    Random,
    /// Every Pokémon learns every one.
    Full,
}

/// The options SoulSync lets a player change before randomizing. `None`
/// keeps what the settings file says.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Overrides {
    pub tm: Option<Compat>,
    pub tutor: Option<Compat>,
    /// Every Pokémon learns every HM.
    pub full_hm: Option<bool>,
}

/// A randomizer settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    /// The randomizer version that wrote it; it updates older ones itself.
    version: u32,
    /// The decoded settings string.
    data: Vec<u8>,
}

impl Preset {
    /// Reads the contents of an `.rnqs` file.
    pub fn read(file: &[u8]) -> Result<Preset, PresetError> {
        if file.len() < 8 {
            return Err(PresetError::Malformed);
        }
        let version = u32::from_be_bytes([file[0], file[1], file[2], file[3]]);
        let len = u32::from_be_bytes([file[4], file[5], file[6], file[7]]) as usize;
        let text = file.get(8..8 + len).ok_or(PresetError::Malformed)?;
        let data = base64_decode(text).ok_or(PresetError::Malformed)?;
        let name = *data.get(OPTIONS).ok_or(PresetError::Malformed)? as usize;
        if data.len() != OPTIONS + 1 + name + 8 {
            return Err(PresetError::Malformed);
        }
        let preset = Preset { version, data };
        if preset.crc() != preset.stored_crc() {
            return Err(PresetError::Checksum);
        }
        Ok(preset)
    }

    /// The contents of an `.rnqs` file with these settings.
    pub fn write(&self) -> Vec<u8> {
        let text = base64_encode(&self.data);
        let mut out = Vec::with_capacity(8 + text.len());
        out.extend_from_slice(&self.version.to_be_bytes());
        out.extend_from_slice(&(text.len() as u32).to_be_bytes());
        out.extend_from_slice(text.as_bytes());
        out
    }

    /// The name of the game the settings were made for, as the randomizer
    /// writes it, e.g. `Pokemon Black 2 (G)`.
    pub fn rom_name(&self) -> String {
        let len = self.data[OPTIONS] as usize;
        String::from_utf8_lossy(&self.data[OPTIONS + 1..OPTIONS + 1 + len]).into_owned()
    }

    fn checked(&self) -> usize {
        self.data.len() - 8
    }

    fn crc(&self) -> u32 {
        crc32(&self.data[..self.checked()])
    }

    fn stored_crc(&self) -> u32 {
        let o = self.checked();
        u32::from_be_bytes([
            self.data[o],
            self.data[o + 1],
            self.data[o + 2],
            self.data[o + 3],
        ])
    }

    fn bit(&self, byte: usize, bit: u8) -> bool {
        self.data[byte] & (1 << bit) != 0
    }

    fn set_bit(&mut self, byte: usize, bit: u8, on: bool) {
        if on {
            self.data[byte] |= 1 << bit;
        } else {
            self.data[byte] &= !(1 << bit);
        }
    }

    /// Both TM and tutor compatibility use bits 0, 1, 2 and 7 of their byte.
    fn compat(&self, byte: usize) -> Compat {
        if self.bit(byte, 7) {
            Compat::Full
        } else if self.bit(byte, 0) {
            Compat::Random
        } else if self.bit(byte, 1) {
            Compat::RandomPreferType
        } else {
            Compat::Unchanged
        }
    }

    fn set_compat(&mut self, byte: usize, compat: Compat) {
        self.set_bit(byte, 0, compat == Compat::Random);
        self.set_bit(byte, 1, compat == Compat::RandomPreferType);
        self.set_bit(byte, 2, compat == Compat::Unchanged);
        self.set_bit(byte, 7, compat == Compat::Full);
    }

    pub fn tm(&self) -> Compat {
        self.compat(TMS)
    }

    pub fn tutor(&self) -> Compat {
        self.compat(TUTORS)
    }

    pub fn full_hm(&self) -> bool {
        self.bit(TMS2, 0)
    }

    /// Whether the moves on the TMs are randomized.
    pub fn random_tm_moves(&self) -> bool {
        self.bit(TMS, 3)
    }

    /// The options as this file has them.
    pub fn options(&self) -> Overrides {
        Overrides {
            tm: Some(self.tm()),
            tutor: Some(self.tutor()),
            full_hm: Some(self.full_hm()),
        }
    }

    /// The same settings with the given options changed.
    pub fn with(&self, overrides: &Overrides) -> Preset {
        let mut out = self.clone();
        if let Some(tm) = overrides.tm {
            out.set_compat(TMS, tm);
        }
        if let Some(tutor) = overrides.tutor {
            out.set_compat(TUTORS, tutor);
        }
        if let Some(full) = overrides.full_hm {
            out.set_bit(TMS2, 0, full);
        }
        let o = out.checked();
        let crc = out.crc().to_be_bytes();
        out.data[o..o + 4].copy_from_slice(&crc);
        out
    }
}

/// A file as Base64, to send it along with a run.
pub fn encode(file: &[u8]) -> String {
    base64_encode(file)
}

/// The file back from [`encode`].
pub fn decode(text: &str) -> Option<Vec<u8>> {
    base64_decode(text.as_bytes())
}

/// CRC-32 as `java.util.zip.CRC32` computes it.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn base64_decode(text: &[u8]) -> Option<Vec<u8>> {
    let text: Vec<u8> = text
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if text.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    for chunk in text.chunks(4) {
        let mut n = 0u32;
        let mut pad = 0;
        for &c in chunk {
            let v = match c {
                b'=' => {
                    pad += 1;
                    0
                }
                _ if pad > 0 => return None,
                _ => ALPHABET.iter().position(|&a| a == c)? as u32,
            };
            n = n << 6 | v;
        }
        if pad > 2 {
            return None;
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..3 - pad]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real settings file for Black 2 (German), saved by randomizer 4.6.0:
    /// TM moves random, TM compatibility unchanged, full HM compatibility.
    const FILE: &str = "AAABQQAAAGxhUWdFQVFMdkFmSUI5UUVFQUFLb0JoZ0NEQUVBREFBZENRQUJBZ0FBQUFBQUFCR0FNZ2t5d1FBSUNUSUdCZ0F5QUFVQUUxQnZhMlZ0YjI0Z1FteGhZMnNnTWlBb1J5bGRsbGNONDhNNGlnPT0=";

    fn file() -> Vec<u8> {
        base64_decode(FILE.as_bytes()).unwrap()
    }

    #[test]
    fn reads_a_real_file() {
        let p = Preset::read(&file()).unwrap();
        assert_eq!(p.rom_name(), "Pokemon Black 2 (G)");
        assert_eq!(p.tm(), Compat::Unchanged);
        assert!(p.random_tm_moves());
        assert!(p.full_hm());
        assert_eq!(p.tutor(), Compat::Unchanged);
        assert_eq!(p.write(), file());
    }

    #[test]
    fn changes_options_and_keeps_the_rest() {
        let p = Preset::read(&file()).unwrap();
        let o = Overrides {
            tm: Some(Compat::RandomPreferType),
            tutor: Some(Compat::Full),
            full_hm: Some(false),
        };
        let q = Preset::read(&p.with(&o).write()).unwrap();
        assert_eq!(q.tm(), Compat::RandomPreferType);
        assert_eq!(q.tutor(), Compat::Full);
        assert!(!q.full_hm());
        assert!(q.random_tm_moves(), "TM moves stay random");
        let changed: Vec<usize> = (0..p.data.len())
            .filter(|&i| p.data[i] != q.data[i])
            .collect();
        assert_eq!(changed[..3], [TMS, TMS2, TUTORS]);
        assert!(
            changed[3..].iter().all(|&i| i >= p.checked()),
            "only the CRC else"
        );
        // Back to what it was gives the same file.
        assert_eq!(q.with(&p.options()).write(), file());
        assert_eq!(p.with(&Overrides::default()), p);
    }

    #[test]
    fn refuses_damaged_files() {
        let mut f = file();
        assert_eq!(Preset::read(&f[..6]), Err(PresetError::Malformed));
        let i = f.len() - 20;
        f[i] = if f[i] == b'A' { b'B' } else { b'A' };
        assert!(Preset::read(&f).is_err());
    }

    #[test]
    fn crc_matches_java() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_decode(b"YWI=").unwrap(), b"ab");
    }
}

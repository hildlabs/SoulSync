//! Takes a DS ROM apart and puts it back together.
//!
//! A randomized game is the player's own ROM with some of its files
//! replaced. This module reads the parts of a ROM, lets files be swapped,
//! and writes a ROM again with every offset and the header checksum set
//! anew. The layout it writes is the one Universal Pokémon Randomizer ZX
//! writes, so the two can be compared byte for byte.

use crate::blz;
use crate::rom::{file_id, u32_at, RomError};
use std::borrow::Cow;

/// Every part of the written ROM starts on a multiple of this.
const ALIGN: usize = 0x200;
const BANNER_LEN: usize = 0x840;
/// The bytes of the header its checksum covers; the checksum follows them.
const HEADER_CRC_AT: usize = 0x15E;
/// The bytes after the ARM9 program that are kept with it: a footer where
/// the game has one.
const ARM9_TAIL: usize = 12;
/// The first word of an ARM9 footer.
const NITROCODE: u32 = 0xDEC0_0621;
const OVERLAY_ENTRY: usize = 32;

/// A ROM in parts, borrowed from the bytes it was read from.
pub struct Nds<'a> {
    header: &'a [u8],
    /// The ARM9 program as stored, with what follows it.
    arm9: Cow<'a, [u8]>,
    /// The length of the program in `arm9`.
    arm9_len: usize,
    /// The ARM9 overlay table.
    y9: Cow<'a, [u8]>,
    arm7: &'a [u8],
    y7: &'a [u8],
    banner: &'a [u8],
    fnt: &'a [u8],
    /// Every file by its id, overlays included.
    files: Vec<Cow<'a, [u8]>>,
}

/// The code of a stored ARM9 program, the footers after it, and where in
/// the code its own packed length is written, if it is packed.
type Arm9Parts<'a> = (&'a [u8], Vec<u8>, Option<usize>);

fn part<'a>(rom: &'a [u8], at: u32, len: usize, what: &'static str) -> Result<&'a [u8], RomError> {
    let at = at as usize;
    at.checked_add(len)
        .and_then(|end| rom.get(at..end))
        .ok_or(RomError::Layout(what))
}

fn align(at: usize) -> usize {
    (at + ALIGN - 1) & !(ALIGN - 1)
}

fn put(out: &mut Vec<u8>, at: usize, data: &[u8]) {
    if out.len() < at + data.len() {
        out.resize(at + data.len(), 0);
    }
    out[at..at + data.len()].copy_from_slice(data);
}

fn put_u32(out: &mut Vec<u8>, at: usize, value: usize) {
    put(out, at, &(value as u32).to_le_bytes());
}

/// The CRC-16 of a DS header.
fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for &byte in data {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

impl<'a> Nds<'a> {
    pub fn parse(rom: &'a [u8]) -> Result<Self, RomError> {
        let field = |at| u32_at(rom, at).ok_or(RomError::Layout("header"));
        let header = part(rom, 0, field(0x84)? as usize, "header")?;
        if header.len() < HEADER_CRC_AT + 2 {
            return Err(RomError::Layout("header"));
        }
        let arm9_len = field(0x2C)? as usize;
        let arm9 = part(rom, field(0x20)?, arm9_len + ARM9_TAIL, "arm9")?;
        let y9 = part(rom, field(0x50)?, field(0x54)? as usize, "overlay table")?;
        let arm7 = part(rom, field(0x30)?, field(0x3C)? as usize, "arm7")?;
        let y7 = part(rom, field(0x58)?, field(0x5C)? as usize, "overlay table")?;
        let banner = part(rom, field(0x68)?, BANNER_LEN, "banner")?;
        let fnt = part(rom, field(0x40)?, field(0x44)? as usize, "file names")?;
        let fat = part(rom, field(0x48)?, field(0x4C)? as usize, "file table")?;
        let files = fat
            .chunks_exact(8)
            .map(|entry| {
                let start = u32_at(entry, 0).unwrap_or(0);
                let end = u32_at(entry, 4).unwrap_or(0);
                let len = end
                    .checked_sub(start)
                    .ok_or(RomError::Layout("file table"))?;
                part(rom, start, len as usize, "file").map(Cow::Borrowed)
            })
            .collect::<Result<_, _>>()?;
        Ok(Nds {
            header,
            arm9: Cow::Borrowed(arm9),
            arm9_len,
            y9: Cow::Borrowed(y9),
            arm7,
            y7,
            banner,
            fnt,
            files,
        })
    }

    /// How many files the ROM has.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The id of the file at `path`, such as `a/0/1/6`.
    pub fn id(&self, path: &str) -> Option<usize> {
        file_id(self.fnt, path).map(usize::from)
    }

    /// The id of the file that holds ARM9 overlay `overlay`.
    pub fn overlay_id(&self, overlay: usize) -> Option<usize> {
        let at = overlay.checked_mul(OVERLAY_ENTRY)?;
        let entry = self.y9.get(at..at + OVERLAY_ENTRY)?;
        u32_at(entry, 24).map(|id| id as usize)
    }

    pub fn file(&self, id: usize) -> Option<&[u8]> {
        self.files.get(id).map(|f| &**f)
    }

    /// Replaces a file; false if the ROM has no file with that id.
    pub fn set_file(&mut self, id: usize, data: Vec<u8>) -> bool {
        match self.files.get_mut(id) {
            Some(file) => {
                *file = Cow::Owned(data);
                true
            }
            None => false,
        }
    }

    /// The stored ARM9 program in its parts.
    fn arm9_parts(&self) -> Result<Arm9Parts<'_>, RomError> {
        let bad = RomError::Layout("arm9");
        let (mut code, tail) = self.arm9.split_at(self.arm9_len);
        let word =
            |code: &[u8], back: usize| code.len().checked_sub(back).and_then(|at| u32_at(code, at));
        let mut footer = if u32_at(tail, 0) == Some(NITROCODE) {
            tail.to_vec()
        } else {
            Vec::new()
        };
        // Footers counted into the program's length.
        while word(code, 12) == Some(NITROCODE)
            || (word(code, 12) == Some(0) && word(code, 8) == Some(0) && word(code, 4) == Some(0))
        {
            let (rest, extra) = code.split_at(code.len() - 12);
            footer.splice(0..0, extra.iter().copied());
            code = rest;
        }
        if code.len() < 8 {
            return Err(bad);
        }
        let mut len_at = None;
        if (8..=0xB).contains(&code[code.len() - 5]) {
            let packed = (word(code, 8).unwrap_or(0) & 0x00FF_FFFF) as usize;
            if packed > code.len() * 9 / 10 && packed < code.len() * 11 / 10 {
                let ram = u32_at(self.header, 0x28).ok_or(RomError::Layout("header"))?;
                let end = (code.len() as u32).wrapping_add(ram).to_le_bytes();
                let mut found = code.windows(4).enumerate().filter(|(_, w)| *w == end);
                len_at = found.next().map(|(at, _)| at);
                if len_at.is_none() || found.next().is_some() {
                    return Err(bad);
                }
            }
        }
        Ok((code, footer, len_at))
    }

    /// The ARM9 program, unpacked.
    pub fn arm9(&self) -> Result<Vec<u8>, RomError> {
        let (code, _, len_at) = self.arm9_parts()?;
        match len_at {
            Some(_) => blz::decode(code).ok_or(RomError::Layout("arm9")),
            None => Ok(code.to_vec()),
        }
    }

    /// Replaces the ARM9 program with `code`, packed if it was before.
    pub fn set_arm9(&mut self, code: &[u8]) -> Result<(), RomError> {
        let (_, footer, len_at) = self.arm9_parts()?;
        let mut stored = match len_at {
            Some(at) => {
                let mut packed = blz::encode(code, true);
                let ram = u32_at(self.header, 0x28).unwrap_or(0);
                let end = (packed.len() as u32).wrapping_add(ram);
                packed
                    .get_mut(at..at + 4)
                    .ok_or(RomError::Layout("arm9"))?
                    .copy_from_slice(&end.to_le_bytes());
                packed
            }
            None => code.to_vec(),
        };
        self.arm9_len = stored.len();
        stored.extend_from_slice(&footer);
        self.arm9 = Cow::Owned(stored);
        Ok(())
    }

    /// The table entry of ARM9 overlay `overlay`, and whether its file is
    /// packed.
    fn overlay_entry(&self, overlay: usize) -> Option<(usize, bool)> {
        let at = overlay.checked_mul(OVERLAY_ENTRY)?;
        let entry = self.y9.get(at..at + OVERLAY_ENTRY)?;
        let file = self.files.get(u32_at(entry, 24)? as usize)?;
        let packed_len = (u32_at(entry, 28)? & 0x00FF_FFFF) as usize;
        Some((
            at,
            entry[31] != 0 && packed_len != 0 && packed_len == file.len(),
        ))
    }

    /// ARM9 overlay `overlay`, unpacked.
    pub fn overlay(&self, overlay: usize) -> Option<Vec<u8>> {
        let (_, packed) = self.overlay_entry(overlay)?;
        let file = self.file(self.overlay_id(overlay)?)?;
        if packed {
            blz::decode(file)
        } else {
            Some(file.to_vec())
        }
    }

    /// Replaces ARM9 overlay `overlay` with `code`, packed if it was before;
    /// false if the ROM has no such overlay.
    pub fn set_overlay(&mut self, overlay: usize, code: &[u8]) -> bool {
        let Some((at, packed)) = self.overlay_entry(overlay) else {
            return false;
        };
        let Some(id) = self.overlay_id(overlay) else {
            return false;
        };
        let stored = if packed {
            let stored = blz::encode(code, false);
            let len = (stored.len() as u32).to_le_bytes();
            self.y9.to_mut()[at + 28..at + 31].copy_from_slice(&len[..3]);
            stored
        } else {
            code.to_vec()
        };
        self.set_file(id, stored)
    }

    /// The ROM with its files as they are now.
    pub fn build(&self) -> Vec<u8> {
        let len: usize = self.files.iter().map(|f| align(f.len())).sum();
        let mut out = Vec::with_capacity(len + (4 << 20));
        out.extend_from_slice(self.header);

        let arm9_at = align(out.len());
        put(&mut out, arm9_at, &self.arm9);
        let y9_at = out.len();
        put(&mut out, y9_at, &self.y9);
        let arm7_at = align(out.len());
        put(&mut out, arm7_at, self.arm7);
        let y7_at = out.len();
        put(&mut out, y7_at, self.y7);
        let banner_at = align(out.len());
        put(&mut out, banner_at, self.banner);
        let fnt_at = align(out.len());
        put(&mut out, fnt_at, self.fnt);
        let fat_at = align(out.len());
        let mut end = fat_at + self.files.len() * 8;
        for (id, file) in self.files.iter().enumerate() {
            let at = align(end);
            put(&mut out, at, file);
            end = at + file.len();
            put_u32(&mut out, fat_at + id * 8, at);
            put_u32(&mut out, fat_at + id * 8 + 4, end);
        }
        // An empty last file still ends where the table says.
        out.resize(end.next_multiple_of(4), 0);

        put_u32(&mut out, 0x20, arm9_at);
        put_u32(&mut out, 0x2C, self.arm9_len);
        put_u32(&mut out, 0x30, arm7_at);
        put_u32(&mut out, 0x3C, self.arm7.len());
        put_u32(&mut out, 0x40, fnt_at);
        put_u32(&mut out, 0x48, fat_at);
        put_u32(&mut out, 0x50, y9_at);
        put_u32(&mut out, 0x58, y7_at);
        put_u32(&mut out, 0x68, banner_at);
        let total = out.len();
        put_u32(&mut out, 0x80, total);
        // The cartridge size the ROM fits on: 128 KiB shifted left by this.
        let chip = out.len().next_power_of_two().max(128 << 10);
        out[0x14] = (chip.trailing_zeros() - 17) as u8;
        let crc = crc16(&out[..HEADER_CRC_AT]);
        put(&mut out, HEADER_CRC_AT, &crc.to_le_bytes());
        out
    }
}

/// The files of a NARC archive, owned so they can be changed.
pub fn narc_unpack(narc: &[u8]) -> Option<Vec<Vec<u8>>> {
    Some(
        crate::rom::narc_entries(narc)?
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect(),
    )
}

/// A NARC archive of `files`, without file names, as the games of
/// generation 5 have them.
pub fn narc_pack(files: &[Vec<u8>]) -> Vec<u8> {
    let data: usize = files.iter().map(|f| f.len().next_multiple_of(4)).sum();
    let btaf = 12 + files.len() * 8;
    let total = 16 + btaf + 16 + 8 + data;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"NARC");
    out.extend_from_slice(&[0xFE, 0xFF, 0x00, 0x01]);
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&[0x10, 0, 3, 0]);
    out.extend_from_slice(b"BTAF");
    out.extend_from_slice(&(btaf as u32).to_le_bytes());
    out.extend_from_slice(&(files.len() as u32).to_le_bytes());
    let mut at = 0usize;
    for file in files {
        out.extend_from_slice(&(at as u32).to_le_bytes());
        out.extend_from_slice(&((at + file.len()) as u32).to_le_bytes());
        at += file.len().next_multiple_of(4);
    }
    out.extend_from_slice(b"BTNF");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&0x10000u32.to_le_bytes());
    out.extend_from_slice(b"GMIF");
    out.extend_from_slice(&((data + 8) as u32).to_le_bytes());
    for file in files {
        out.extend_from_slice(file);
        out.resize(out.len().next_multiple_of(4), 0xFF);
    }
    out
}

/// A small ROM laid out like a game's, for tests here and elsewhere.
#[cfg(test)]
pub(crate) fn sample(files: &[(&str, &[u8])], overlays: &[&[u8]]) -> Vec<u8> {
    // File names: one folder `a` under the root holding every file. Overlays
    // come first and have no names.
    let first = overlays.len() as u16;
    let mut fnt = Vec::new();
    fnt.extend_from_slice(&16u32.to_le_bytes());
    fnt.extend_from_slice(&first.to_le_bytes());
    fnt.extend_from_slice(&2u16.to_le_bytes());
    let root: &[u8] = &[0x81, b'a', 0x01, 0xF0, 0];
    fnt.extend_from_slice(&(16 + root.len() as u32).to_le_bytes());
    fnt.extend_from_slice(&first.to_le_bytes());
    fnt.extend_from_slice(&0xF000u16.to_le_bytes());
    fnt.extend_from_slice(root);
    for (name, _) in files {
        fnt.push(name.len() as u8);
        fnt.extend_from_slice(name.as_bytes());
    }
    fnt.push(0);

    let mut rom = vec![0u8; 0x4000];
    rom[..12].copy_from_slice(b"POKEMON B2\0\0");
    rom[12..16].copy_from_slice(b"IRED");
    put_u32(&mut rom, 0x84, 0x4000);
    let add = |rom: &mut Vec<u8>, at_field: usize, len_field: Option<usize>, data: &[u8]| {
        // Parts sit at odd places on purpose: building has to move them.
        let at = rom.len() + 3;
        put(rom, at, data);
        put_u32(rom, at_field, at);
        if let Some(field) = len_field {
            put_u32(rom, field, data.len());
        }
    };
    let mut arm9 = vec![9u8; 0x123];
    arm9.extend_from_slice(&0xDEC0_0621u32.to_le_bytes());
    arm9.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    add(&mut rom, 0x20, None, &arm9);
    put_u32(&mut rom, 0x2C, 0x123);
    add(&mut rom, 0x30, Some(0x3C), &[7u8; 0x77]);
    add(&mut rom, 0x68, None, &[0xBA; BANNER_LEN]);
    add(&mut rom, 0x40, Some(0x44), &fnt);
    let mut y9 = Vec::new();
    for (i, overlay) in overlays.iter().enumerate() {
        let mut entry = [0u8; OVERLAY_ENTRY];
        entry[..4].copy_from_slice(&(i as u32).to_le_bytes());
        entry[8..12].copy_from_slice(&(overlay.len() as u32).to_le_bytes());
        entry[24..28].copy_from_slice(&(i as u32).to_le_bytes());
        y9.extend_from_slice(&entry);
    }
    add(&mut rom, 0x50, Some(0x54), &y9);
    add(&mut rom, 0x58, Some(0x5C), &[]);
    let mut fat = Vec::new();
    let all = overlays.iter().chain(files.iter().map(|(_, data)| data));
    for data in all {
        let at = rom.len() + 1;
        put(&mut rom, at, data);
        fat.extend_from_slice(&(at as u32).to_le_bytes());
        fat.extend_from_slice(&((at + data.len()) as u32).to_le_bytes());
    }
    add(&mut rom, 0x48, Some(0x4C), &fat);
    rom
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::u16_at;

    fn rom() -> Vec<u8> {
        sample(
            &[("one", b"first file"), ("two", b""), ("three", &[3u8; 700])],
            &[b"overlay zero", &[1u8; 0x333]],
        )
    }

    #[test]
    fn a_rebuilt_rom_holds_the_same_parts() {
        let rom = rom();
        let nds = Nds::parse(&rom).unwrap();
        assert_eq!(nds.len(), 5);
        assert_eq!(nds.id("a/one"), Some(2));
        assert_eq!(nds.overlay_id(1), Some(1));
        assert_eq!(nds.overlay_id(2), None);
        let built = nds.build();
        let again = Nds::parse(&built).unwrap();
        assert_eq!(again.arm9, nds.arm9);
        assert_eq!(again.y9, nds.y9);
        assert_eq!(again.arm7, nds.arm7);
        assert_eq!(again.banner, nds.banner);
        assert_eq!(again.fnt, nds.fnt);
        assert_eq!(again.files, nds.files);
        // Building what was built changes nothing more.
        assert_eq!(again.build(), built);
    }

    #[test]
    fn parts_are_aligned_and_the_header_is_signed() {
        let built = Nds::parse(&rom()).unwrap().build();
        let field = |at| u32_at(&built, at).unwrap() as usize;
        for at in [0x20, 0x30, 0x40, 0x48, 0x68] {
            assert_eq!(field(at) % ALIGN, 0, "part at {at:#x}");
        }
        let fat = field(0x48);
        for id in 0..5 {
            assert_eq!(field(fat + id * 8) % ALIGN, 0);
        }
        assert_eq!(field(0x80), built.len());
        assert_eq!(built.len() % 4, 0);
        // 128 KiB is the smallest cartridge.
        assert_eq!(built[0x14], 0);
        assert_eq!(
            u16_at(&built, HEADER_CRC_AT),
            Some(crc16(&built[..HEADER_CRC_AT]))
        );
        // The check value of this CRC-16, known as MODBUS.
        assert_eq!(crc16(b"123456789"), 0x4B37);
    }

    #[test]
    fn a_replaced_file_moves_the_ones_after_it() {
        let rom = rom();
        let mut nds = Nds::parse(&rom).unwrap();
        let id = nds.id("a/one").unwrap();
        assert!(nds.set_file(id, vec![0xAB; 5000]));
        assert!(!nds.set_file(99, Vec::new()));
        let built = nds.build();
        let again = Nds::parse(&built).unwrap();
        assert_eq!(again.file(id), Some(&[0xAB; 5000][..]));
        assert_eq!(again.file(again.id("a/two").unwrap()), Some(&[][..]));
        assert_eq!(
            again.file(again.id("a/three").unwrap()),
            Some(&[3u8; 700][..])
        );
        assert_eq!(again.file(0), Some(&b"overlay zero"[..]));
    }

    #[test]
    fn a_cut_off_rom_is_refused() {
        let rom = rom();
        assert!(Nds::parse(&rom[..rom.len() - 40]).is_err());
        assert!(Nds::parse(&rom[..0x100]).is_err());
    }

    #[test]
    fn program_code_is_unpacked_and_packed_again() {
        let code: Vec<u8> = (0..3000u32).map(|i| (i % 7) as u8).collect();
        let packed = blz::encode(&code, false);
        let mut rom = sample(&[("one", b"file")], &[&packed, b"plain overlay"]);
        // Mark overlay 0 as packed in its table entry.
        let y9 = u32_at(&rom, 0x50).unwrap() as usize;
        rom[y9 + 28..y9 + 31].copy_from_slice(&(packed.len() as u32).to_le_bytes()[..3]);
        rom[y9 + 31] = 1;

        let mut nds = Nds::parse(&rom).unwrap();
        assert_eq!(nds.overlay(0), Some(code));
        assert_eq!(nds.overlay(1), Some(b"plain overlay".to_vec()));
        assert_eq!(nds.overlay(2), None);
        let longer: Vec<u8> = (0..5000u32).map(|i| (i % 11) as u8).collect();
        assert!(nds.set_overlay(0, &longer));
        assert!(nds.set_overlay(1, b"new plain overlay"));
        assert!(!nds.set_overlay(2, b""));
        assert_eq!(nds.arm9().unwrap(), vec![9u8; 0x123]);
        nds.set_arm9(&[8u8; 0x200]).unwrap();

        let built = nds.build();
        let again = Nds::parse(&built).unwrap();
        assert_eq!(again.overlay(0), Some(longer));
        assert_ne!(again.file(0).unwrap().len(), 5000, "stored packed");
        assert_eq!(again.overlay(1), Some(b"new plain overlay".to_vec()));
        assert_eq!(again.arm9().unwrap(), vec![8u8; 0x200]);
        // The footer follows the new program.
        assert_eq!(&again.arm9[0x200..], &nds.arm9[0x200..]);
        assert_eq!(u32_at(&again.arm9, 0x200), Some(NITROCODE));
    }

    #[test]
    fn a_narc_packs_and_unpacks() {
        let files = vec![b"abc".to_vec(), Vec::new(), vec![7u8; 8], vec![1u8; 5]];
        let narc = narc_pack(&files);
        assert_eq!(u32_at(&narc, 8), Some(narc.len() as u32));
        assert_eq!(narc.len() % 4, 0);
        assert_eq!(narc_unpack(&narc), Some(files));
        assert_eq!(narc_unpack(b"NARC"), None);
    }
}

//! Reads game data out of a player's own ROM.
//!
//! A randomizer or ROM hack can change species data, so the numbers a run
//! needs come from the ROM the player actually plays. Only the few bytes
//! asked for are read; nothing of the ROM is stored or sent on.

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};

/// Species data of Black, White, Black 2 and White 2.
const PERSONAL: &str = "a/0/1/6";
/// Offset of the catch rate in a species entry.
const CATCH_RATE: usize = 8;
const SPECIES: usize = 649;
/// No file this module reads comes near this size.
const MAX_LEN: u32 = 4 << 20;

#[derive(Debug)]
pub enum RomError {
    Io(io::Error),
    /// The file is not laid out like a DS ROM with generation 5 data.
    Layout(&'static str),
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RomError::Io(e) => write!(f, "{e}"),
            RomError::Layout(what) => write!(f, "not a readable generation 5 ROM ({what})"),
        }
    }
}

impl std::error::Error for RomError {}

impl From<io::Error> for RomError {
    fn from(e: io::Error) -> Self {
        RomError::Io(e)
    }
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

fn read_at<R: Read + Seek>(rom: &mut R, offset: u32, len: u32) -> Result<Vec<u8>, RomError> {
    if len > MAX_LEN {
        return Err(RomError::Layout("oversized table"));
    }
    rom.seek(SeekFrom::Start(offset.into()))?;
    let mut buf = vec![0; len as usize];
    rom.read_exact(&mut buf)?;
    Ok(buf)
}

/// Finds a file in the ROM's file name table and returns its file id.
fn file_id(fnt: &[u8], path: &str) -> Option<u16> {
    let mut dir = 0usize;
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        let is_last = parts.peek().is_none();
        let mut at = u32_at(fnt, dir * 8)? as usize;
        let mut id = u16_at(fnt, dir * 8 + 4)?;
        let mut found = None;
        loop {
            let head = *fnt.get(at)?;
            if head == 0 {
                break;
            }
            let (is_dir, len) = (head & 0x80 != 0, (head & 0x7F) as usize);
            let name = fnt.get(at + 1..at + 1 + len)?;
            at += 1 + len;
            if is_dir {
                let sub = u16_at(fnt, at)?;
                at += 2;
                if !is_last && name == part.as_bytes() {
                    found = Some(sub);
                    break;
                }
            } else {
                if is_last && name == part.as_bytes() {
                    return Some(id);
                }
                id = id.checked_add(1)?;
            }
        }
        dir = (found?.checked_sub(0xF000)?) as usize;
    }
    None
}

/// Reads one file of the ROM's file system.
fn read_file<R: Read + Seek>(rom: &mut R, path: &str) -> Result<Vec<u8>, RomError> {
    let header = read_at(rom, 0, 0x50)?;
    let field = |at| u32_at(&header, at).ok_or(RomError::Layout("header"));
    let (fnt_at, fnt_len, fat_at, fat_len) =
        (field(0x40)?, field(0x44)?, field(0x48)?, field(0x4C)?);
    let fnt = read_at(rom, fnt_at, fnt_len)?;
    let id = u32::from(file_id(&fnt, path).ok_or(RomError::Layout("file not found"))?);
    if (id + 1) * 8 > fat_len {
        return Err(RomError::Layout("file table"));
    }
    let entry = read_at(rom, fat_at + id * 8, 8)?;
    let (start, end) = (
        u32_at(&entry, 0).unwrap_or(0),
        u32_at(&entry, 4).unwrap_or(0),
    );
    let len = end
        .checked_sub(start)
        .ok_or(RomError::Layout("file table"))?;
    read_at(rom, start, len)
}

/// The entries of a NARC archive.
fn narc_entries(narc: &[u8]) -> Option<Vec<&[u8]>> {
    if narc.get(..4)? != b"NARC" {
        return None;
    }
    let btaf = u16_at(narc, 0x0C)? as usize;
    if narc.get(btaf..btaf + 4)? != b"BTAF" {
        return None;
    }
    let count = u16_at(narc, btaf + 8)? as usize;
    let btnf = btaf + u32_at(narc, btaf + 4)? as usize;
    let gmif = btnf + u32_at(narc, btnf + 4)? as usize;
    if narc.get(gmif..gmif + 4)? != b"GMIF" {
        return None;
    }
    let data = narc.get(gmif + 8..)?;
    (0..count)
        .map(|i| {
            let at = btaf + 12 + i * 8;
            data.get(u32_at(narc, at)? as usize..u32_at(narc, at + 4)? as usize)
        })
        .collect()
}

/// Catch rates as the ROM has them, by National Dex number: index 0 is
/// species 1.
pub fn read_catch_rates<R: Read + Seek>(rom: &mut R) -> Result<Vec<u8>, RomError> {
    let narc = read_file(rom, PERSONAL)?;
    let entries = narc_entries(&narc).ok_or(RomError::Layout("species archive"))?;
    // Entry 0 is a placeholder; forms follow after the last species.
    let rates: Vec<u8> = entries
        .iter()
        .skip(1)
        .take(SPECIES)
        .filter_map(|e| e.get(CATCH_RATE).copied())
        .collect();
    if rates.len() == SPECIES {
        Ok(rates)
    } else {
        Err(RomError::Layout("species archive"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn narc(entries: &[Vec<u8>]) -> Vec<u8> {
        let mut btaf = b"BTAF".to_vec();
        btaf.extend((12 + entries.len() as u32 * 8).to_le_bytes());
        btaf.extend((entries.len() as u32).to_le_bytes());
        let mut data: Vec<u8> = Vec::new();
        for e in entries {
            btaf.extend((data.len() as u32).to_le_bytes());
            data.extend(e);
            btaf.extend((data.len() as u32).to_le_bytes());
        }
        let mut out = b"NARC\xFE\xFF\x00\x01\0\0\0\0\x10\x00\x03\x00".to_vec();
        out.extend(btaf);
        out.extend(b"BTNF\x10\0\0\0\x04\0\0\0\0\0\x01\0");
        out.extend(b"GMIF");
        out.extend((data.len() as u32 + 8).to_le_bytes());
        out.extend(data);
        out
    }

    /// A ROM with the files `a/0/0/0` .. and `a/0/1/6` holding `personal`.
    fn rom(personal: &[u8]) -> Vec<u8> {
        // Directories: 0 root, 1 "a", 2 "0", 3 "0", 4 "1".
        let dir = |name: &str, id: u16| {
            let mut v = vec![0x80 | name.len() as u8];
            v.extend(name.bytes());
            v.extend(id.to_le_bytes());
            v.push(0);
            v
        };
        let files = |n: u8| {
            let mut v = Vec::new();
            for i in 0..n {
                v.extend([1, b'0' + i]);
            }
            v.push(0);
            v
        };
        let mut second = dir("0", 0xF003);
        second.pop();
        second.extend(dir("1", 0xF004));
        let subs = [
            dir("a", 0xF001),
            dir("0", 0xF002),
            second,
            files(3),
            files(7),
        ];
        let first_file = [0u16, 0, 0, 0, 3];
        let mut fnt = Vec::new();
        let mut at = 5 * 8;
        for (sub, first) in subs.iter().zip(first_file) {
            fnt.extend((at as u32).to_le_bytes());
            fnt.extend(first.to_le_bytes());
            fnt.extend(5u16.to_le_bytes());
            at += sub.len();
        }
        subs.iter().for_each(|s| fnt.extend(s));

        let fnt_at = 0x200u32;
        let fat_at = fnt_at + fnt.len() as u32;
        let data_at = fat_at + 10 * 8;
        let mut fat = Vec::new();
        for id in 0..10u32 {
            let (start, end) = if id == 9 {
                (data_at, data_at + personal.len() as u32)
            } else {
                (data_at, data_at)
            };
            fat.extend(start.to_le_bytes());
            fat.extend(end.to_le_bytes());
        }
        let mut out = vec![0u8; fnt_at as usize];
        out[0x40..0x44].copy_from_slice(&fnt_at.to_le_bytes());
        out[0x44..0x48].copy_from_slice(&(fnt.len() as u32).to_le_bytes());
        out[0x48..0x4C].copy_from_slice(&fat_at.to_le_bytes());
        out[0x4C..0x50].copy_from_slice(&(fat.len() as u32).to_le_bytes());
        out.extend(fnt);
        out.extend(fat);
        out.extend(personal);
        out
    }

    #[test]
    fn reads_catch_rates() {
        let entries: Vec<Vec<u8>> = (0..=700u32)
            .map(|i| {
                let mut e = vec![0u8; 0x4C];
                e[CATCH_RATE] = (i % 251) as u8;
                e
            })
            .collect();
        let rates = read_catch_rates(&mut Cursor::new(rom(&narc(&entries)))).unwrap();
        assert_eq!(rates.len(), 649);
        assert_eq!((rates[0], rates[1], rates[648]), (1, 2, (649 % 251) as u8));
    }

    #[test]
    fn rejects_other_files() {
        assert!(read_catch_rates(&mut Cursor::new(vec![0u8; 16])).is_err());
        assert!(read_catch_rates(&mut Cursor::new(vec![0xFFu8; 0x1000])).is_err());
        let short = narc(&vec![vec![0u8; 0x4C]; 10]);
        assert!(read_catch_rates(&mut Cursor::new(rom(&short))).is_err());
    }
}

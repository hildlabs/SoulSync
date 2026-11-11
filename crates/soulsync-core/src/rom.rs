//! Reads game data out of a player's own ROM.
//!
//! A randomizer or ROM hack can change species data, so the numbers a run
//! needs come from the ROM the player actually plays. Only the few bytes
//! asked for are read; nothing of the ROM is stored or sent on.

use crate::game::Game;
use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};

/// Species data of Black, White, Black 2 and White 2.
const PERSONAL: &str = "a/0/1/6";
/// The map headers, one file with a record per map.
const MAPS: &str = "a/0/1/2";
const MAP_HEADER: usize = 48;
const MAP_LOCATION: usize = 26;
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

/// The location of every map, by map id: the index of the location's name,
/// which is also what a Pokémon caught there gets as met location.
pub fn read_map_locations<R: Read + Seek>(rom: &mut R) -> Result<Vec<u16>, RomError> {
    let narc = read_file(rom, MAPS)?;
    let table = narc_entries(&narc)
        .and_then(|e| e.first().copied())
        .ok_or(RomError::Layout("map table"))?;
    if table.is_empty() || table.len() % MAP_HEADER != 0 {
        return Err(RomError::Layout("map table"));
    }
    Ok(table
        .chunks(MAP_HEADER)
        .map(|h| u16::from(h[MAP_LOCATION]))
        .collect())
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

/// Where a game keeps its trainers, and whose teams set the level caps.
struct Trainers {
    data: &'static str,
    parties: &'static str,
    /// The gym leaders in badge order, then the Pokémon League. A stage can
    /// have several trainers, such as the three leaders of the first gym in
    /// Black and White.
    stages: [&'static [u16]; 9],
}

const TRAINERS_BW: Trainers = Trainers {
    data: "a/0/9/2",
    parties: "a/0/9/3",
    stages: [
        &[0x0B, 0x0C, 0x0D],
        &[0x15],
        &[0x16],
        &[0x17],
        &[0x18],
        &[0x19],
        &[0x83],
        &[0x84, 0x85],
        &[0xE4, 0xE5, 0xE6, 0xE7],
    ],
};

const TRAINERS_B2W2: Trainers = Trainers {
    data: "a/0/9/1",
    parties: "a/0/9/2",
    stages: [
        &[0x9C],
        &[0x9D],
        &[0x9A],
        &[0x99],
        &[0x9E],
        &[0x9B],
        &[0x9F],
        &[0xA0],
        &[0x26, 0x27, 0x28, 0x29, 0x155],
    ],
};

/// Level of a trainer's strongest Pokémon.
fn ace(data: &[&[u8]], parties: &[&[u8]], trainer: u16) -> Option<u8> {
    let (info, party) = (
        data.get(usize::from(trainer))?,
        parties.get(usize::from(trainer))?,
    );
    let (flags, count) = (*info.first()?, usize::from(*info.get(3)?));
    // Each Pokémon takes 8 bytes, plus 2 with a held item and 8 with moves.
    let size = 8 + if flags & 2 != 0 { 2 } else { 0 } + if flags & 1 != 0 { 8 } else { 0 };
    (0..count)
        .map(|i| party.get(i * size + 2).copied())
        .collect::<Option<Vec<u8>>>()?
        .into_iter()
        .max()
}

/// Level caps as the ROM has them: the level of the strongest Pokémon of
/// each gym leader in badge order, then of the Pokémon League. A randomizer
/// or ROM hack can change these levels, so they are read, not assumed.
pub fn read_level_caps<R: Read + Seek>(rom: &mut R, game: Game) -> Result<Vec<u8>, RomError> {
    let trainers = match game {
        Game::Black | Game::White => &TRAINERS_BW,
        Game::Black2 | Game::White2 => &TRAINERS_B2W2,
    };
    let bad = RomError::Layout("trainer archive");
    let (data, parties) = (
        read_file(rom, trainers.data)?,
        read_file(rom, trainers.parties)?,
    );
    let (Some(data), Some(parties)) = (narc_entries(&data), narc_entries(&parties)) else {
        return Err(bad);
    };
    trainers
        .stages
        .iter()
        .map(|stage| {
            let aces: Option<Vec<u8>> = stage.iter().map(|&t| ace(&data, &parties, t)).collect();
            aces.and_then(|a| a.into_iter().max())
                .filter(|level| (1..=100).contains(level))
        })
        .collect::<Option<Vec<u8>>>()
        .ok_or(bad)
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

    /// A ROM with the folders `a/0/0` to `a/0/9`, ten files `0` to `9` in
    /// each, and the given contents at paths such as `a/0/1/6`.
    fn rom(contents: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let dir = |name: &str, id: u16| {
            let mut v = vec![0x80 | name.len() as u8];
            v.extend(name.bytes());
            v.extend(id.to_le_bytes());
            v
        };
        // Directories: 0 root, 1 "a", 2 "0", then 3 to 12 for "0" to "9".
        let mut subs = vec![dir("a", 0xF001), dir("0", 0xF002), Vec::new()];
        for d in 0..10u16 {
            subs[2].extend(dir(&d.to_string(), 0xF003 + d));
        }
        for _ in 0..10 {
            subs.push((0..10u8).flat_map(|f| [1, b'0' + f]).collect());
        }
        subs.iter_mut().for_each(|s| s.push(0));
        let mut fnt = Vec::new();
        let mut at = subs.len() * 8;
        for (i, sub) in subs.iter().enumerate() {
            fnt.extend((at as u32).to_le_bytes());
            fnt.extend((i.saturating_sub(3) as u16 * 10).to_le_bytes());
            fnt.extend((subs.len() as u16).to_le_bytes());
            at += sub.len();
        }
        subs.iter().for_each(|s| fnt.extend(s));

        let fnt_at = 0x200u32;
        let fat_at = fnt_at + fnt.len() as u32;
        let mut at = fat_at + 100 * 8;
        let (mut fat, mut data) = (Vec::new(), Vec::<u8>::new());
        for id in 0..100usize {
            let path = format!("a/0/{}/{}", id / 10, id % 10);
            let file = contents.iter().find(|(p, _)| *p == path);
            let len = file.map_or(0, |(_, d)| d.len() as u32);
            fat.extend(at.to_le_bytes());
            fat.extend((at + len).to_le_bytes());
            at += len;
            if let Some((_, d)) = file {
                data.extend(d);
            }
        }
        let mut out = vec![0u8; fnt_at as usize];
        out[0x40..0x44].copy_from_slice(&fnt_at.to_le_bytes());
        out[0x44..0x48].copy_from_slice(&(fnt.len() as u32).to_le_bytes());
        out[0x48..0x4C].copy_from_slice(&fat_at.to_le_bytes());
        out[0x4C..0x50].copy_from_slice(&(fat.len() as u32).to_le_bytes());
        out.extend(fnt);
        out.extend(fat);
        out.extend(data);
        out
    }

    #[test]
    fn reads_map_locations() {
        let mut table = vec![0u8; 3 * MAP_HEADER];
        table[MAP_LOCATION] = 4;
        table[MAP_HEADER + MAP_LOCATION] = 14;
        table[2 * MAP_HEADER + MAP_LOCATION] = 14;
        let image = rom(&[("a/0/1/2", narc(&[table]))]);
        assert_eq!(
            read_map_locations(&mut Cursor::new(image)).unwrap(),
            vec![4, 14, 14]
        );
        let image = rom(&[("a/0/1/2", narc(&[vec![0u8; 50]]))]);
        assert!(read_map_locations(&mut Cursor::new(image)).is_err());
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
        let rom = rom(&[("a/0/1/6", narc(&entries))]);
        let rates = read_catch_rates(&mut Cursor::new(rom)).unwrap();
        assert_eq!(rates.len(), 649);
        assert_eq!((rates[0], rates[1], rates[648]), (1, 2, (649 % 251) as u8));
    }

    /// Trainer archives in which trainer `id` has the given levels; `flags`
    /// says whether the Pokémon carry items (2) and moves (1).
    fn trainers(teams: &[(u16, u8, &[u8])]) -> (Vec<u8>, Vec<u8>) {
        let (mut data, mut parties) = (vec![vec![0u8; 20]; 0x160], vec![Vec::new(); 0x160]);
        for &(id, flags, levels) in teams {
            data[usize::from(id)][0] = flags;
            data[usize::from(id)][3] = levels.len() as u8;
            let size = 8 + if flags & 2 != 0 { 2 } else { 0 } + if flags & 1 != 0 { 8 } else { 0 };
            for &level in levels {
                let mut poke = vec![0xEE; size];
                poke[2] = level;
                parties[usize::from(id)].extend(poke);
            }
        }
        (narc(&data), narc(&parties))
    }

    #[test]
    fn reads_level_caps() {
        let b2w2: [(u16, u8, &[u8]); 13] = [
            (0x9C, 0, &[11, 13]),
            (0x9D, 2, &[16, 18]),
            (0x9A, 1, &[22, 22, 24]),
            (0x99, 3, &[28, 28, 30]),
            (0x9E, 3, &[31, 33, 31]),
            (0x9B, 3, &[37, 37, 39]),
            (0x9F, 3, &[46, 46, 48]),
            (0xA0, 3, &[49, 49, 51]),
            (0x26, 3, &[56, 58]),
            (0x27, 3, &[56, 58]),
            (0x28, 3, &[56, 58]),
            (0x29, 3, &[56, 58]),
            (0x155, 3, &[57, 57, 59, 57]),
        ];
        let (data, parties) = trainers(&b2w2);
        let image = rom(&[("a/0/9/1", data.clone()), ("a/0/9/2", parties.clone())]);
        let caps = read_level_caps(&mut Cursor::new(&image), Game::Black2).unwrap();
        assert_eq!(caps, [13, 18, 24, 30, 33, 39, 48, 51, 59]);
        // The same files mean other trainers in Black and White.
        assert!(read_level_caps(&mut Cursor::new(&image), Game::Black).is_err());
        // A leader without Pokémon is not a cap.
        let (data, parties) = trainers(&b2w2[1..]);
        let image = rom(&[("a/0/9/1", data), ("a/0/9/2", parties)]);
        assert!(read_level_caps(&mut Cursor::new(&image), Game::White2).is_err());
    }
}

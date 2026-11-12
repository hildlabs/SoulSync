//! Text archives of generation 5 games: every name and description the game
//! shows, in the ROM's own language.
//!
//! A text file holds sections (one per language slot; these games use one)
//! of entries. Each entry is a run of UTF-16 units, encrypted with a key that
//! starts from its last unit and turns three bits per unit, back to front.

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// Reads all entries of the first section of a text file.
pub fn read(file: &[u8]) -> Option<Vec<String>> {
    let sections = u16_at(file, 0)?;
    let entries = usize::from(u16_at(file, 2)?);
    if sections == 0 {
        return None;
    }
    let base = u32_at(file, 0x0C)? as usize;
    u32_at(file, base)?;
    // An entry that cannot be read stays empty instead of losing the file.
    let entry = |i: usize| -> Option<String> {
        let at = base + 4 + i * 8;
        let offset = base + u32_at(file, at)? as usize;
        let len = usize::from(u16_at(file, at + 4)?);
        let units: Option<Vec<u16>> = (0..len).map(|j| u16_at(file, offset + 2 * j)).collect();
        Some(decode(&decrypt(&units?)))
    };
    Some((0..entries).map(|i| entry(i).unwrap_or_default()).collect())
}

fn decrypt(units: &[u16]) -> Vec<u16> {
    let Some(&last) = units.last() else {
        return Vec::new();
    };
    let mut key = last ^ 0xFFFF;
    let mut out = vec![0; units.len()];
    for (i, &u) in units.iter().enumerate().rev() {
        out[i] = u ^ key;
        key = key.rotate_right(3);
    }
    out
}

/// Turns decrypted units into a plain string: line breaks become spaces and
/// the game's placeholders and formatting codes are left out.
fn decode(units: &[u16]) -> String {
    let mut text = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        match units[i] {
            0xFFFF => break,
            0xFFFE => text.push(u16::from(b' ')),
            // A code: its number, how many arguments follow, the arguments.
            0xF000 => {
                let args = units.get(i + 2).copied().unwrap_or(0);
                i += 2 + usize::from(args);
            }
            // Compressed text, only found in a few debug strings.
            0xF100 => return String::new(),
            0x246D => text.push(0x2642), // ♂
            0x246E => text.push(0x2640), // ♀
            u => text.push(u),
        }
        i += 1;
    }
    let s = String::from_utf16_lossy(&text);
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Builds a text file the way the games store one. For tests.
#[cfg(test)]
pub(crate) fn build(entries: &[&str]) -> Vec<u8> {
    let mut table = Vec::new();
    let mut data = Vec::new();
    let head = 4 + entries.len() * 8;
    for e in entries {
        let mut units: Vec<u16> = e.encode_utf16().collect();
        units.push(0xFFFF);
        // The last plain unit is 0xFFFF, so its encrypted form, xor 0xFFFF,
        // is the key it was encrypted with: any key works.
        let mut key: u16 = 0x2983;
        let mut enc = vec![0u16; units.len()];
        for (i, &u) in units.iter().enumerate().rev() {
            enc[i] = u ^ key;
            key = key.rotate_right(3);
        }
        table.extend(((head + data.len()) as u32).to_le_bytes());
        table.extend((enc.len() as u16).to_le_bytes());
        table.extend(0u16.to_le_bytes());
        for u in enc {
            data.extend(u.to_le_bytes());
        }
    }
    let mut section = ((head + data.len()) as u32).to_le_bytes().to_vec();
    section.extend(table);
    section.extend(data);
    let mut file = Vec::new();
    file.extend(1u16.to_le_bytes());
    file.extend((entries.len() as u16).to_le_bytes());
    file.extend((section.len() as u32).to_le_bytes());
    file.extend(0u32.to_le_bytes());
    file.extend(0x10u32.to_le_bytes());
    file.extend(section);
    file
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_other_entries_when_one_is_broken() {
        let mut file = build(&["Tackle", "Growl"]);
        // The second entry now points past the end of the file.
        let at = 0x10 + 4 + 8;
        file[at..at + 4].copy_from_slice(&0xFFFFu32.to_le_bytes());
        assert_eq!(read(&file).unwrap(), ["Tackle", ""]);
    }

    #[test]
    fn reads_what_it_wrote() {
        let file = build(&["Psychokinese", "Ein starker\u{FFFE}Angriff."]);
        assert_eq!(
            read(&file).unwrap(),
            ["Psychokinese", "Ein starker Angriff."]
        );
    }

    #[test]
    fn leaves_out_codes() {
        let units = [0x0041, 0xF000, 0x0100, 0x0001, 0x0000, 0x0042, 0xFFFF];
        assert_eq!(decode(&units), "AB");
    }
}

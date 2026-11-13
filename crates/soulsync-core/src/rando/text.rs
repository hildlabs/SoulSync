//! Reads and rewrites the text files of a generation 5 game the way the
//! original randomizer does, so a rewritten file comes out the same.
//!
//! A line is kept as the game's own 16-bit characters without its end
//! mark; names the randomizer changes are plain letters in them.

/// The mark at the end of a line.
const END: u16 = 0xFFFF;
/// The first character of a line stored with 9 bits per character.
const PACKED: u16 = 0xF100;

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?) as usize)
}

/// Symbols the game has characters of its own for. The original randomizer
/// shows those as the plain symbol and writes every such symbol back as the
/// game's character, also where the text had the plain one.
const SYMBOLS: [(u16, u16); 19] = [
    (0x00D7, 0x2467),
    (0x00F7, 0x2468),
    (0x2026, 0x246C),
    (0x2642, 0x246D),
    (0x2640, 0x246E),
    (0x2660, 0x246F),
    (0x2663, 0x2470),
    (0x2665, 0x2471),
    (0x2666, 0x2472),
    (0x2605, 0x2473),
    (0x25CE, 0x2474),
    (0x25CB, 0x2475),
    (0x25A1, 0x2476),
    (0x25B3, 0x2477),
    (0x25C7, 0x2478),
    (0x266A, 0x2479),
    (0x2600, 0x247A),
    (0x2601, 0x247B),
    (0x2602, 0x247D),
];

fn symbol(c: u16) -> u16 {
    SYMBOLS
        .iter()
        .find(|(plain, _)| *plain == c)
        .map_or(c, |(_, own)| *own)
}

/// The lines of a text file, with what is needed to write them again.
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub lines: Vec<Vec<u16>>,
    /// The key each line's first character is encrypted with.
    keys: Vec<u16>,
    /// A value stored with each line that the game wants back.
    extras: Vec<u16>,
}

/// Unpacks a line of 9-bit characters.
fn unpack(words: &[u16]) -> Option<Vec<u16>> {
    let mut out = Vec::new();
    let mut at = 1;
    let mut shift = 0u32;
    let mut carry = 0u16;
    let keep = |out: &mut Vec<u16>, c: u16| {
        if c > 1 {
            out.push(c);
        }
    };
    loop {
        let word = *words.get(at)?;
        if shift >= 16 {
            shift -= 16;
            if shift > 0 {
                let c = carry | ((u32::from(word) << (9 - shift)) & 0x1FF) as u16;
                if c & 0xFF == 0xFF {
                    return Some(out);
                }
                keep(&mut out, c);
            }
        } else {
            let c = (word >> shift) & 0x1FF;
            if c & 0xFF == 0xFF {
                return Some(out);
            }
            keep(&mut out, c);
            shift += 9;
            if shift < 16 {
                carry = (word >> shift) & 0x1FF;
                shift += 9;
            }
            at += 1;
        }
    }
}

impl Text {
    pub fn read(file: &[u8]) -> Option<Text> {
        let sections = u16_at(file, 0)?;
        let count = usize::from(u16_at(file, 2)?);
        let mut text = Text {
            lines: Vec::with_capacity(count),
            keys: Vec::with_capacity(count),
            extras: Vec::with_capacity(count),
        };
        if sections == 0 {
            return Some(text);
        }
        let section = u32_at(file, 12)?;
        for i in 0..count {
            let entry = section + 4 + i * 8;
            let at = section + u32_at(file, entry)?;
            let len = usize::from(u16_at(file, entry + 4)?);
            text.extras.push(u16_at(file, entry + 6)?);
            let mut words: Vec<u16> = (0..len)
                .map(|k| u16_at(file, at + k * 2))
                .collect::<Option<_>>()?;
            let mut key = *words.last()? ^ END;
            for word in words.iter_mut().rev() {
                *word ^= key;
                key = key.rotate_right(3);
            }
            // After the last turn back the key is one step before the first.
            text.keys.push(key.rotate_left(3));
            if words[0] == PACKED {
                words = unpack(&words)?;
            }
            words.retain(|&c| c != END);
            text.lines.push(words);
        }
        Some(text)
    }

    /// The file `original` with these lines in place of its own. Lines are
    /// never stored packed, as the original randomizer never does.
    pub fn write(&self, original: &[u8]) -> Option<Vec<u8>> {
        let sections = u16_at(original, 0)?;
        let count = usize::from(u16_at(original, 2)?);
        if sections == 0 || self.lines.len() < count {
            return Some(original.to_vec());
        }
        let mut lines: Vec<Vec<u16>> = (0..count)
            .map(|i| {
                let mut key = self.keys[i];
                let end = std::iter::once(END);
                (self.lines[i].iter().copied().chain(end))
                    .map(|c| {
                        let c = symbol(c) ^ key;
                        key = key.rotate_left(3);
                        c
                    })
                    .collect()
            })
            .collect();
        let chars: usize = lines.iter().map(Vec::len).sum();
        if chars % 2 == 1 {
            // Sections are whole 32-bit words long: one more end mark.
            if let Some(last) = lines.last_mut() {
                let key = self.keys[count - 1].rotate_left(3 * last.len() as u32 % 16);
                last.push(END ^ key);
            }
        }
        let table = 4 + 8 * count;
        let size = table + lines.iter().map(Vec::len).sum::<usize>() * 2;
        let mut section = Vec::with_capacity(size);
        section.extend_from_slice(&(size as u32).to_le_bytes());
        let mut at = table;
        for (line, extra) in lines.iter().zip(&self.extras) {
            section.extend_from_slice(&(at as u32).to_le_bytes());
            section.extend_from_slice(&(line.len() as u16).to_le_bytes());
            section.extend_from_slice(&extra.to_le_bytes());
            at += line.len() * 2;
        }
        for word in lines.iter().flatten() {
            section.extend_from_slice(&word.to_le_bytes());
        }

        let first = u32_at(original, 12)?;
        let old_size = u32_at(original, first)?;
        let mut out = original.to_vec();
        out.resize((original.len() + size).checked_sub(old_size)?, 0);
        out[4..8].copy_from_slice(&(size as u32).to_le_bytes());
        out.get_mut(first..first + size)?.copy_from_slice(&section);
        if sections == 2 {
            let second = u32_at(original, 16)?;
            let second_size = u32_at(original, second)?;
            let moved = first + size;
            out[16..20].copy_from_slice(&(moved as u32).to_le_bytes());
            out.get_mut(moved..moved + second_size)?
                .copy_from_slice(original.get(second..second + second_size)?);
        }
        Some(out)
    }
}

/// The characters of `s` as the game stores plain text.
pub fn chars(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file of one section with `lines`, each encrypted from `key` on.
    fn file(lines: &[&[u16]], key: u16) -> Vec<u8> {
        let text = Text {
            lines: lines.iter().map(|l| l.to_vec()).collect(),
            keys: vec![key; lines.len()],
            extras: (0..lines.len() as u16).collect(),
        };
        // A file whose one section has no lines yet.
        let mut empty = vec![1, 0];
        empty.extend_from_slice(&(lines.len() as u16).to_le_bytes());
        empty.extend_from_slice(&[4, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 4, 0, 0, 0]);
        text.write(&empty).unwrap()
    }

    #[test]
    fn lines_are_read_back_as_written() {
        let lines: [&[u16]; 3] = [&chars("Bisasam"), &[], &chars("Glurak")];
        let data = file(&lines, 0x1234);
        assert_eq!(data.len() % 4, 0);
        let text = Text::read(&data).unwrap();
        assert_eq!(text.lines, lines.map(<[u16]>::to_vec));
        assert_eq!(text.keys, [0x1234; 3]);
        assert_eq!(text.extras, [0, 1, 2]);
        // Unchanged lines give the same file.
        assert_eq!(text.write(&data).unwrap(), data);
    }

    #[test]
    fn a_longer_line_moves_the_rest() {
        let data = file(&[&chars("a"), &chars("b")], 7);
        let mut text = Text::read(&data).unwrap();
        text.lines[0] = chars("a much longer line");
        let longer = text.write(&data).unwrap();
        assert!(longer.len() > data.len());
        assert_eq!(Text::read(&longer).unwrap().lines, text.lines);
    }

    #[test]
    fn packed_lines_unpack() {
        // "AB" in 9 bits per character: 0x041, 0x042, then the end 0x1FF.
        let bits: u32 = 0x041 | 0x042 << 9 | 0x1FF << 18;
        let words = [PACKED, bits as u16, (bits >> 16) as u16, END];
        assert_eq!(unpack(&words), Some(chars("AB")));
    }

    #[test]
    fn a_cut_off_file_is_refused() {
        let data = file(&[&chars("abc")], 1);
        assert_eq!(Text::read(&data[..data.len() - 3]), None);
    }
}

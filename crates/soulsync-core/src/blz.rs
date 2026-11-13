//! The compression of a DS game's program code ("bottom LZ").
//!
//! The ARM9 program and its overlays are stored compressed from the end
//! towards the start, so the console can unpack them in place. This is a
//! port of CUE's BLZ coder in its normal mode, as Universal Pokémon
//! Randomizer ZX uses it, and packs to the same bytes.

/// A match is worth coding from this length on.
const THRESHOLD: usize = 2;
/// How far back a match may start.
const WINDOW: usize = 0x1002;
/// The longest match.
const LONGEST: usize = 0x12;
const RAW_MAX: usize = 0x00FF_FFFF;
/// The start of an ARM9 program stays unpacked: it holds the code that
/// unpacks the rest.
const ARM9_PLAIN: usize = 0x4000;

fn u32_at(data: &[u8], at: usize) -> usize {
    usize::from(data[at])
        | usize::from(data[at + 1]) << 8
        | usize::from(data[at + 2]) << 16
        | usize::from(data[at + 3] & 0x7F) << 24
}

/// Unpacks `data`; `None` if it is not laid out like packed code. Code that
/// was stored unpacked comes back as it is.
pub fn decode(data: &[u8]) -> Option<Vec<u8>> {
    let len = data.len();
    if len < 4 {
        return None;
    }
    let grown = u32_at(data, len - 4);
    if grown == 0 {
        return Some(data.to_vec());
    }
    if len < 8 {
        return None;
    }
    let header = usize::from(data[len - 5]);
    if !(8..=0xB).contains(&header) || len <= header {
        return None;
    }
    let packed = u32_at(data, len - 8) & 0x00FF_FFFF;
    let plain = len.checked_sub(packed)?;
    let body = packed.checked_sub(header)?;
    let total = plain + packed + grown;
    if total > RAW_MAX {
        return None;
    }
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&data[..plain]);
    // The packed part is read from its end; what it unpacks to is written
    // from the end as well, so it is collected here and turned at last.
    let mut pak = data[plain..plain + body].iter().rev().copied();
    let mut flags = 0u8;
    let mut mask = 0u8;
    'unpack: while out.len() < total {
        mask >>= 1;
        if mask == 0 {
            let Some(next) = pak.next() else { break };
            flags = next;
            mask = 0x80;
        }
        if flags & mask == 0 {
            let Some(byte) = pak.next() else { break };
            out.push(byte);
        } else {
            let (Some(high), Some(low)) = (pak.next(), pak.next()) else {
                break;
            };
            let pair = usize::from(high) << 8 | usize::from(low);
            let count = ((pair >> 12) + THRESHOLD + 1).min(total - out.len());
            let back = (pair & 0xFFF) + 3;
            for _ in 0..count {
                // A match never reaches into the unpacked start.
                let Some(at) = out.len().checked_sub(back).filter(|&at| at >= plain) else {
                    break 'unpack;
                };
                out.push(out[at]);
            }
        }
    }
    out[plain..].reverse();
    Some(out)
}

/// The longest match for the bytes at `at` among those before it, as
/// (length, distance); `back` is the distance kept when none is longer than
/// the threshold.
fn search(raw: &[u8], at: usize, end: usize, mut back: usize) -> (usize, usize) {
    let mut best = THRESHOLD;
    for pos in 3..=at.min(WINDOW) {
        let limit = LONGEST.min(end - at).min(pos);
        let len = (0..limit)
            .take_while(|&i| raw[at + i] == raw[at + i - pos])
            .count();
        if len > best {
            back = pos;
            best = len;
            if best == LONGEST {
                break;
            }
        }
    }
    (best, back)
}

/// Packs `data`. With `arm9` its first 0x4000 bytes stay unpacked.
pub fn encode(data: &[u8], arm9: bool) -> Vec<u8> {
    let len = data.len();
    let raw: Vec<u8> = data.iter().rev().copied().collect();
    let end = if arm9 {
        len.saturating_sub(ARM9_PLAIN)
    } else {
        len
    };
    let mut pak: Vec<u8> = Vec::with_capacity(len + len / 8 + 11);
    let (mut at, mut flag, mut mask, mut back) = (0usize, 0usize, 0u8, 0usize);
    // The point up to which packing paid off most: packed bytes so far and
    // raw bytes left.
    let (mut pak_best, mut raw_best) = (0usize, len);
    while at < end {
        mask >>= 1;
        if mask == 0 {
            flag = pak.len();
            pak.push(0);
            mask = 0x80;
        }
        let (found, pos) = search(&raw, at, end, back);
        back = pos;
        pak[flag] <<= 1;
        if found > THRESHOLD {
            at += found;
            pak[flag] |= 1;
            pak.push(((found - (THRESHOLD + 1)) << 4 | (pos - 3) >> 8) as u8);
            pak.push(((pos - 3) & 0xFF) as u8);
        } else {
            pak.push(raw[at]);
            at += 1;
        }
        if pak.len() + len - at < pak_best + raw_best {
            pak_best = pak.len();
            raw_best = len - at;
        }
    }
    while mask > 1 {
        mask >>= 1;
        pak[flag] <<= 1;
    }
    pak.reverse();

    if pak_best == 0 || len + 4 < ((pak_best + raw_best + 3) & !3) + 8 {
        // Packing does not pay: the code as it is, marked as not packed.
        let mut out = data.to_vec();
        out.resize(len.next_multiple_of(4), 0);
        out.extend_from_slice(&[0; 4]);
        return out;
    }
    let mut out = Vec::with_capacity(raw_best + pak_best + 11);
    out.extend_from_slice(&data[..raw_best]);
    out.extend_from_slice(&pak[pak.len() - pak_best..]);
    let mut header = 8;
    while out.len() % 4 != 0 {
        out.push(0xFF);
        header += 1;
    }
    let packed = (pak_best + header) as u32;
    out.extend_from_slice(&packed.to_le_bytes()[..3]);
    out.push(header as u8);
    let grown = (len - pak_best - raw_best - header) as u32;
    out.extend_from_slice(&(grown & 0x7FFF_FFFF).to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes with repeats, as code has them.
    fn code(len: usize, seed: u32) -> Vec<u8> {
        let mut x = seed;
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let word = [(x >> 16) as u8 & 0x1F, (x >> 8) as u8 & 3, 0xE5, 0x9F];
            let times = 1 + (x >> 28) as usize % 4;
            for _ in 0..times {
                out.extend_from_slice(&word);
            }
        }
        out.truncate(len);
        out
    }

    #[test]
    fn packed_code_unpacks_to_what_it_was() {
        for (len, seed) in [(1, 1), (37, 2), (1000, 3), (5001, 4), (20_000, 5)] {
            let data = code(len, seed);
            let packed = encode(&data, false);
            assert_eq!(packed.len() % 4, 0);
            let unpacked = decode(&packed).unwrap();
            // Code too short to pack is padded to whole words.
            assert_eq!(&unpacked[..len], &data[..], "length {len}");
            assert!(unpacked[len..].iter().all(|&b| b == 0));
            if len >= 1000 {
                assert_eq!(unpacked.len(), len);
                assert!(packed.len() < len, "length {len} got smaller");
            }
        }
    }

    #[test]
    fn the_start_of_an_arm9_stays_plain() {
        let data = code(ARM9_PLAIN + 9000, 6);
        let packed = encode(&data, true);
        assert_eq!(&packed[..ARM9_PLAIN], &data[..ARM9_PLAIN]);
        assert_eq!(decode(&packed).unwrap(), data);
    }

    #[test]
    fn what_is_not_packed_code_is_refused() {
        assert_eq!(decode(&[1, 2, 3]), None);
        // A header length outside 8 to 11.
        assert_eq!(decode(&[0, 0, 0, 0, 9, 0, 0, 7, 1, 0, 0, 0]), None);
        // More packed bytes than the file has.
        assert_eq!(decode(&[0xFF, 0xFF, 0, 8, 1, 0, 0, 0]), None);
        // Matches that point before the data end the unpacking early.
        let mut bad = vec![0xFF; 16];
        bad.extend_from_slice(&[24, 0, 0, 8, 100, 0, 0, 0]);
        assert!(decode(&bad).is_some());
    }
}

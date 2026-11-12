//! Pokémon sprites and item icons out of a player's own ROM, as PNG.
//!
//! SoulSync ships no game graphics. The app draws them from the ROM the
//! player chose, on their own machine, and serves them only to its own
//! window.
//!
//! The sprite layout follows the Universal Pokémon Randomizer ZX
//! (`Gen5RomHandler.getMascotImage`): every species has 20 files in
//! `a/0/0/4`, the first is its front sprite, file 18 its palette.

use crate::rom::{narc_entries, read_file, RomError};
use std::io::{Read, Seek};

/// The sprites, 20 files per species.
const POKEGRA: &str = "a/0/0/4";
const PER_SPECIES: usize = 20;
const PALETTE: usize = 18;
/// The item icons, a graphic and its palette per item.
const ITEMGRA: &str = "a/0/2/5";

/// The two archives, read once.
pub struct Art {
    pokegra: Vec<u8>,
    items: Option<Vec<u8>>,
}

impl Art {
    pub fn read<R: Read + Seek>(rom: &mut R) -> Result<Self, RomError> {
        let pokegra = read_file(rom, POKEGRA)?;
        if narc_entries(&pokegra).is_none() {
            return Err(RomError::Layout("sprite archive"));
        }
        let items = read_file(rom, ITEMGRA)
            .ok()
            .filter(|n| narc_entries(n).is_some());
        Ok(Art { pokegra, items })
    }

    /// The front sprite of a species, 96 × 96.
    pub fn sprite(&self, species: u16) -> Option<Vec<u8>> {
        let files = narc_entries(&self.pokegra)?;
        let at = usize::from(species) * PER_SPECIES;
        let pic = unpack(files.get(at)?)?;
        let pal = palette(files.get(at + PALETTE)?)?;
        // The 96 × 96 picture is stored as a 64 × 144 strip of tiles.
        let strip = tiled(pic.get(48..)?, &pal, 64, 144)?;
        let mut out = vec![0u8; 96 * 96 * 4];
        let mut blit = |dx: usize, dy: usize, sx: usize, sy: usize, w: usize, h: usize| {
            for y in 0..h {
                let s = ((sy + y) * 64 + sx) * 4;
                let d = ((dy + y) * 96 + dx) * 4;
                out[d..d + w * 4].copy_from_slice(&strip[s..s + w * 4]);
            }
        };
        blit(0, 0, 0, 0, 64, 64);
        for k in 0..8 {
            blit(64, k * 8, (k % 2) * 32, 64 + (k / 2) * 8, 32, 8);
        }
        blit(0, 64, 0, 96, 64, 32);
        for k in 0..4 {
            blit(64, 64 + k * 8, (k % 2) * 32, 128 + (k / 2) * 8, 32, 8);
        }
        Some(png(96, 96, &out))
    }

    /// The bag icon of an item, by its index in the game.
    pub fn item_icon(&self, item: u16) -> Option<Vec<u8>> {
        let files = narc_entries(self.items.as_ref()?)?;
        let at = usize::from(item) * 2;
        let pic = unpack(files.get(at)?)?;
        let pal = palette(files.get(at + 1)?)?;
        if pic.get(..4)? != b"RGCN" {
            return None;
        }
        let (h, w) = (u16_at(&pic, 0x18)?, u16_at(&pic, 0x1A)?);
        let (w, h) = if w == 0xFFFF || h == 0xFFFF || w == 0 || h == 0 || w > 16 || h > 16 {
            (32, 32)
        } else {
            (usize::from(w) * 8, usize::from(h) * 8)
        };
        let linear = u32_at(&pic, 0x24)? & 1 == 1;
        let data = pic.get(48..)?;
        let rgba = if linear {
            scanned(data, &pal, w, h)?
        } else {
            tiled(data, &pal, w, h)?
        };
        Some(png(w as u32, h as u32, &rgba))
    }
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// A file as stored, or unpacked when it is LZ compressed.
fn unpack(file: &[u8]) -> Option<Vec<u8>> {
    match file.first()? {
        0x10 | 0x11 if file.get(..4) != Some(b"RGCN") => lz(file),
        _ => Some(file.to_vec()),
    }
}

/// The 16 colours of an NCLR palette; colour 0 is see-through.
fn palette(file: &[u8]) -> Option<[[u8; 4]; 16]> {
    let file = unpack(file)?;
    let mut out = [[0u8; 4]; 16];
    for (i, c) in out.iter_mut().enumerate().skip(1) {
        let v = u16_at(&file, 40 + i * 2)?;
        let ch = |x: u16| ((x & 0x1F) as u32 * 33 / 4).min(255) as u8;
        *c = [ch(v), ch(v >> 5), ch(v >> 10), 255];
    }
    Some(out)
}

/// A 4 bpp picture made of 8 × 8 tiles, row by row.
fn tiled(data: &[u8], pal: &[[u8; 4]; 16], w: usize, h: usize) -> Option<Vec<u8>> {
    if data.len() < w * h / 2 {
        return None;
    }
    let mut out = vec![0u8; w * h * 4];
    let across = w / 8;
    for tile in 0..w * h / 64 {
        let (tx, ty) = (tile % across, tile / across);
        for y in 0..8 {
            for x in 0..8 {
                let byte = data[tile * 32 + y * 4 + x / 2];
                let v = if x % 2 == 0 { byte & 0xF } else { byte >> 4 };
                let at = ((ty * 8 + y) * w + tx * 8 + x) * 4;
                out[at..at + 4].copy_from_slice(&pal[usize::from(v)]);
            }
        }
    }
    Some(out)
}

/// A 4 bpp picture stored line by line.
fn scanned(data: &[u8], pal: &[[u8; 4]; 16], w: usize, h: usize) -> Option<Vec<u8>> {
    if data.len() < w * h / 2 {
        return None;
    }
    let mut out = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let byte = data[i / 2];
        let v = if i % 2 == 0 { byte & 0xF } else { byte >> 4 };
        out[i * 4..i * 4 + 4].copy_from_slice(&pal[usize::from(v)]);
    }
    Some(out)
}

/// Nintendo's LZ77 variants 0x10 and 0x11.
fn lz(src: &[u8]) -> Option<Vec<u8>> {
    let kind = *src.first()?;
    let mut size = (u32_at(src, 0)? >> 8) as usize;
    let mut i = 4;
    if size == 0 {
        size = u32_at(src, 4)? as usize;
        i = 8;
    }
    if size > 1 << 24 {
        return None;
    }
    let mut out = Vec::with_capacity(size);
    while out.len() < size {
        let flags = *src.get(i)?;
        i += 1;
        for bit in (0..8).rev() {
            if out.len() >= size {
                break;
            }
            if flags >> bit & 1 == 0 {
                out.push(*src.get(i)?);
                i += 1;
                continue;
            }
            let b0 = usize::from(*src.get(i)?);
            let b1 = usize::from(*src.get(i + 1)?);
            let (len, disp);
            if kind == 0x10 {
                len = (b0 >> 4) + 3;
                disp = ((b0 & 0xF) << 8 | b1) + 1;
                i += 2;
            } else {
                match b0 >> 4 {
                    0 => {
                        let b2 = usize::from(*src.get(i + 2)?);
                        len = ((b0 & 0xF) << 4 | b1 >> 4) + 0x11;
                        disp = ((b1 & 0xF) << 8 | b2) + 1;
                        i += 3;
                    }
                    1 => {
                        let b2 = usize::from(*src.get(i + 2)?);
                        let b3 = usize::from(*src.get(i + 3)?);
                        len = ((b0 & 0xF) << 12 | b1 << 4 | b2 >> 4) + 0x111;
                        disp = ((b2 & 0xF) << 8 | b3) + 1;
                        i += 4;
                    }
                    n => {
                        len = n + 1;
                        disp = ((b0 & 0xF) << 8 | b1) + 1;
                        i += 2;
                    }
                }
            }
            if disp > out.len() {
                return None;
            }
            for _ in 0..len {
                out.push(out[out.len() - disp]);
            }
        }
    }
    out.truncate(size);
    Some(out)
}

/// An RGBA picture as PNG, stored without compression: the icons are tiny.
pub fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(rgba.len() + h as usize);
    for row in rgba.chunks(w as usize * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (k, b) in blocks.iter().enumerate() {
        z.push(u8::from(k + 1 == blocks.len()));
        let n = b.len() as u16;
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&(b << 16 | a).to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in [(b"IHDR", ihdr), (b"IDAT", z), (b"IEND", Vec::new())] {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(&data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    out
}

fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &x in d {
        c ^= u32::from(x);
        for _ in 0..8 {
            c = if c & 1 == 1 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_lz10_and_lz11() {
        // "abcabcabc": three literals, then a copy of six from three back.
        let lz10 = [0x10, 9, 0, 0, 0b0001_0000, b'a', b'b', b'c', 0x30, 0x02];
        assert_eq!(lz(&lz10).unwrap(), b"abcabcabc");
        let lz11 = [0x11, 9, 0, 0, 0b0001_0000, b'a', b'b', b'c', 0x50, 0x02];
        assert_eq!(lz(&lz11).unwrap(), b"abcabcabc");
        assert!(lz(&[0x10, 4, 0, 0, 0x80, 0x00, 0x05]).is_none());
    }

    #[test]
    fn draws_tiles_in_order() {
        let mut pal = [[0u8; 4]; 16];
        pal[1] = [255, 0, 0, 255];
        // Two tiles side by side: the first empty, the second filled.
        let mut data = vec![0u8; 64];
        data[32..].fill(0x11);
        let px = tiled(&data, &pal, 16, 8).unwrap();
        assert_eq!(&px[..4], &[0, 0, 0, 0]);
        assert_eq!(&px[8 * 4..8 * 4 + 4], &[255, 0, 0, 255]);
    }

    #[test]
    fn writes_a_png() {
        let p = png(2, 2, &[255; 16]);
        assert_eq!(&p[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&p[12..16], b"IHDR");
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }
}

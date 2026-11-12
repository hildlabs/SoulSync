//! Changing the bag of a Black 2 / White 2 save.
//!
//! The save is split into blocks, each with a CRC-16 after its data and a
//! copy of it in a table near the end, which has its own CRC-16. The game
//! treats a block whose checksum does not match as corrupt, so every change
//! writes all three anew. Offsets as PKHeX documents them for B2W2 and
//! confirmed on a real save.

use crate::save::SaveError;

const SAVE_SIZE: usize = 0x80000;
/// The bag block: all pockets.
const BAG: usize = 0x18400;
const BAG_LEN: usize = 0x9EC;
const BAG_CRC: usize = 0x18DEE;
/// Where the table keeps the bag's CRC (block 25).
const BAG_CRC_COPY: usize = 0x25F32;
/// The table of block checksums, and its own checksum.
const TABLE: usize = 0x25F00;
const TABLE_LEN: usize = 0x94;
const TABLE_CRC: usize = 0x25FA2;

/// The medicine pocket: 48 slots of item number and count (`u16` each).
const MEDICINE: usize = 0x18BD8;
const MEDICINE_SLOTS: usize = 48;

/// Rare Candy.
pub const RARE_CANDY: u16 = 50;
/// The most of one item a slot holds.
pub const MAX_COUNT: u16 = 999;

fn u16_at(save: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([save[o], save[o + 1]])
}

fn put_u16(save: &mut [u8], o: usize, v: u16) {
    save[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

/// CRC-16/CCITT-FALSE, the checksum of generation 5 save blocks.
pub fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for &b in bytes {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Whether the bag block and the checksum table are intact.
fn intact(save: &[u8]) -> bool {
    let bag = crc16(&save[BAG..BAG + BAG_LEN]);
    bag == u16_at(save, BAG_CRC)
        && bag == u16_at(save, BAG_CRC_COPY)
        && crc16(&save[TABLE..TABLE + TABLE_LEN]) == u16_at(save, TABLE_CRC)
}

fn seal(save: &mut [u8]) {
    let bag = crc16(&save[BAG..BAG + BAG_LEN]);
    put_u16(save, BAG_CRC, bag);
    put_u16(save, BAG_CRC_COPY, bag);
    let table = crc16(&save[TABLE..TABLE + TABLE_LEN]);
    put_u16(save, TABLE_CRC, table);
}

/// How many of a medicine item the bag holds.
pub fn medicine(save: &[u8], item: u16) -> u16 {
    if save.len() < SAVE_SIZE {
        return 0;
    }
    (0..MEDICINE_SLOTS)
        .map(|i| MEDICINE + 4 * i)
        .find(|&o| u16_at(save, o) == item)
        .map_or(0, |o| u16_at(save, o + 2))
}

/// Puts exactly `count` of a medicine item, such as [`RARE_CANDY`], into
/// the bag (at most [`MAX_COUNT`]); `0` takes them all out. Works on a raw
/// save and on a DeSmuME `.dsv` alike.
pub fn set_medicine(save: &mut [u8], item: u16, count: u16) -> Result<(), SaveError> {
    if save.len() < SAVE_SIZE {
        return Err(SaveError::TooSmall(save.len()));
    }
    // A save whose checksums are already wrong is not a B2W2 save, or a
    // broken one: better leave it alone than seal whatever is in it.
    if !intact(save) {
        return Err(SaveError::NotB2W2);
    }
    let count = count.min(MAX_COUNT);
    let slot = |i: usize| MEDICINE + 4 * i;
    let items: Vec<(u16, u16)> = (0..MEDICINE_SLOTS)
        .map(|i| (u16_at(save, slot(i)), u16_at(save, slot(i) + 2)))
        .take_while(|&(it, _)| it != 0)
        .collect();
    let mut items: Vec<(u16, u16)> = items.into_iter().filter(|&(it, _)| it != item).collect();
    if count > 0 {
        if items.len() >= MEDICINE_SLOTS {
            return Err(SaveError::NotB2W2);
        }
        // Where it was, or at the end like the game puts a new item.
        let at = (0..MEDICINE_SLOTS)
            .find(|&i| u16_at(save, slot(i)) == item)
            .unwrap_or(items.len())
            .min(items.len());
        items.insert(at, (item, count));
    }
    for i in 0..MEDICINE_SLOTS {
        let (it, n) = items.get(i).copied().unwrap_or((0, 0));
        put_u16(save, slot(i), it);
        put_u16(save, slot(i) + 2, n);
    }
    seal(save);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty save whose bag and table checksums are right.
    fn blank() -> Vec<u8> {
        let mut s = vec![0u8; SAVE_SIZE + 122];
        seal(&mut s);
        s
    }

    #[test]
    fn crc_is_ccitt_false() {
        assert_eq!(crc16(b"123456789"), 0x29B1);
    }

    #[test]
    fn puts_rare_candies_in_and_keeps_the_save_valid() {
        let mut s = blank();
        put_u16(&mut s, MEDICINE, 17); // Potion
        put_u16(&mut s, MEDICINE + 2, 3);
        seal(&mut s);
        set_medicine(&mut s, RARE_CANDY, 999).unwrap();
        assert!(intact(&s));
        assert_eq!(medicine(&s, RARE_CANDY), 999);
        assert_eq!(medicine(&s, 17), 3);
        assert_eq!(u16_at(&s, MEDICINE + 4), RARE_CANDY);

        // Again with another count: same slot, nothing doubled.
        set_medicine(&mut s, RARE_CANDY, 5000).unwrap();
        assert_eq!(medicine(&s, RARE_CANDY), MAX_COUNT);
        assert_eq!(u16_at(&s, MEDICINE + 8), 0);

        set_medicine(&mut s, RARE_CANDY, 0).unwrap();
        assert_eq!(medicine(&s, RARE_CANDY), 0);
        assert_eq!(medicine(&s, 17), 3);
        assert!(intact(&s));
        // The DeSmuME footer is never touched.
        assert!(s[SAVE_SIZE..].iter().all(|&b| b == 0));
    }

    #[test]
    fn leaves_broken_saves_alone() {
        let mut s = blank();
        s[BAG] = 1;
        let before = s.clone();
        assert_eq!(set_medicine(&mut s, RARE_CANDY, 1), Err(SaveError::NotB2W2));
        assert_eq!(s, before);
        assert!(set_medicine(&mut [0u8; 16], RARE_CANDY, 1).is_err());
    }
}

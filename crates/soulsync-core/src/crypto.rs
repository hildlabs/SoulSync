//! Ver- und Entschlüsselung von Gen-5-Pokémon-Datensätzen (PK5).
//!
//! Ein Datensatz besteht aus einem 8-Byte-Kopf (PID, Prüfsumme), vier
//! 32-Byte-Blöcken, die abhängig von der PID vertauscht und mit einem
//! linearen Kongruenzgenerator verschlüsselt sind, und bei Team-Pokémon
//! zusätzlich 84 Byte Kampfwerten, die mit der PID als Seed verschlüsselt sind.

/// Größe eines Box-Datensatzes.
pub const STORED_SIZE: usize = 136;
/// Größe eines Team-Datensatzes (Box-Datensatz plus Kampfwerte).
pub const PARTY_SIZE: usize = 220;

const BLOCK: usize = 32;

/// Für jede der 24 Vertauschungen: welcher gespeicherte Block an Position 0..4 gehört.
const UNSHUFFLE: [[u8; 4]; 24] = [
    [0, 1, 2, 3],
    [0, 1, 3, 2],
    [0, 2, 1, 3],
    [0, 3, 1, 2],
    [0, 2, 3, 1],
    [0, 3, 2, 1],
    [1, 0, 2, 3],
    [1, 0, 3, 2],
    [2, 0, 1, 3],
    [3, 0, 1, 2],
    [2, 0, 3, 1],
    [3, 0, 2, 1],
    [1, 2, 0, 3],
    [1, 3, 0, 2],
    [2, 1, 0, 3],
    [3, 1, 0, 2],
    [2, 3, 0, 1],
    [3, 2, 0, 1],
    [1, 2, 3, 0],
    [1, 3, 2, 0],
    [2, 1, 3, 0],
    [3, 1, 2, 0],
    [2, 3, 1, 0],
    [3, 2, 1, 0],
];

fn shuffle_index(pid: u32) -> usize {
    (((pid >> 13) & 31) % 24) as usize
}

/// XOR mit dem Gen-4/5-LCG, zwei Byte pro Schritt.
fn crypt(data: &mut [u8], mut seed: u32) {
    for chunk in data.chunks_exact_mut(2) {
        seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
        let k = (seed >> 16) as u16;
        let v = u16::from_le_bytes([chunk[0], chunk[1]]) ^ k;
        chunk.copy_from_slice(&v.to_le_bytes());
    }
}

/// Prüfsumme über die 128 Byte Blockdaten (Summe der u16-Werte).
pub fn checksum(blocks: &[u8]) -> u16 {
    blocks.chunks_exact(2).fold(0u16, |s, c| {
        s.wrapping_add(u16::from_le_bytes([c[0], c[1]]))
    })
}

/// Entschlüsselt einen Datensatz (136 oder 220 Byte). Die Blöcke liegen danach
/// in der Reihenfolge A, B, C, D. Gibt `None` zurück bei falscher Länge.
pub fn decrypt(raw: &[u8]) -> Option<Vec<u8>> {
    if raw.len() != STORED_SIZE && raw.len() != PARTY_SIZE {
        return None;
    }
    let mut d = raw.to_vec();
    let pid = u32::from_le_bytes(d[0..4].try_into().ok()?);
    let chk = u16::from_le_bytes(d[6..8].try_into().ok()?);
    crypt(&mut d[8..STORED_SIZE], chk as u32);
    if d.len() == PARTY_SIZE {
        crypt(&mut d[STORED_SIZE..], pid);
    }
    let order = UNSHUFFLE[shuffle_index(pid)];
    let mut out = d.clone();
    for (pos, &src) in order.iter().enumerate() {
        let s = 8 + src as usize * BLOCK;
        out[8 + pos * BLOCK..8 + (pos + 1) * BLOCK].copy_from_slice(&d[s..s + BLOCK]);
    }
    Some(out)
}

/// Gegenstück zu [`decrypt`]: vertauscht und verschlüsselt einen Datensatz in
/// Reihenfolge A, B, C, D. Die Prüfsumme im Kopf wird neu berechnet.
pub fn encrypt(plain: &[u8]) -> Option<Vec<u8>> {
    if plain.len() != STORED_SIZE && plain.len() != PARTY_SIZE {
        return None;
    }
    let mut p = plain.to_vec();
    let chk = checksum(&p[8..STORED_SIZE]);
    p[6..8].copy_from_slice(&chk.to_le_bytes());
    let pid = u32::from_le_bytes(p[0..4].try_into().ok()?);
    let order = UNSHUFFLE[shuffle_index(pid)];
    let mut out = p.clone();
    for (pos, &dst) in order.iter().enumerate() {
        let d = 8 + dst as usize * BLOCK;
        out[d..d + BLOCK].copy_from_slice(&p[8 + pos * BLOCK..8 + (pos + 1) * BLOCK]);
    }
    crypt(&mut out[8..STORED_SIZE], chk as u32);
    if out.len() == PARTY_SIZE {
        crypt(&mut out[STORED_SIZE..], pid);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unshuffle_table_is_permutations() {
        for row in UNSHUFFLE {
            let mut s = row;
            s.sort();
            assert_eq!(s, [0, 1, 2, 3]);
        }
    }

    #[test]
    fn roundtrip_all_shuffles() {
        for sv in 0..24u32 {
            let pid = (sv << 13) | 0x1234_0007;
            let mut plain = vec![0u8; PARTY_SIZE];
            plain[0..4].copy_from_slice(&pid.to_le_bytes());
            for (i, b) in plain.iter_mut().enumerate().skip(8) {
                *b = (i * 7 + sv as usize) as u8;
            }
            let enc = encrypt(&plain).unwrap();
            assert_ne!(enc[8..], plain[8..]);
            let dec = decrypt(&enc).unwrap();
            assert_eq!(dec[8..], plain[8..], "shuffle {sv}");
            assert_eq!(
                checksum(&dec[8..STORED_SIZE]),
                u16::from_le_bytes([dec[6], dec[7]])
            );
        }
    }
}

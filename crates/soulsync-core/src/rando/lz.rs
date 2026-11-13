//! Unpacks the two LZ formats the DS uses for pictures (types 0x10 and
//! 0x11), the way the original randomizer does.

/// The unpacked data, or `None` if `data` is not packed this way or is
/// cut off.
pub fn decode(data: &[u8]) -> Option<Vec<u8>> {
    let kind = *data.first()?;
    if kind != 0x10 && kind != 0x11 {
        return None;
    }
    let mut len = usize::from(*data.get(1)?)
        | usize::from(*data.get(2)?) << 8
        | usize::from(*data.get(3)?) << 16;
    let mut at = 4;
    if len == 0 {
        len = u32::from_be_bytes(data.get(4..8)?.try_into().ok()?) as usize;
        at = 8;
    }
    let mut next = || {
        let byte = data.get(at).copied();
        at += 1;
        byte.map(usize::from)
    };
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let flags = next()?;
        for bit in 0..8 {
            if out.len() >= len {
                break;
            }
            if flags & (0x80 >> bit) == 0 {
                out.push(next()? as u8);
                continue;
            }
            let first = next()?;
            let (count, back) = if kind == 0x10 {
                ((first >> 4) + 3, (first & 15) << 8 | next()?)
            } else {
                match first >> 4 {
                    0 => {
                        let second = next()?;
                        (
                            (first << 4 | second >> 4) + 0x11,
                            (second & 15) << 8 | next()?,
                        )
                    }
                    1 => {
                        let (second, third) = (next()?, next()?);
                        (
                            ((first & 15) << 12 | second << 4 | third >> 4) + 0x111,
                            (third & 15) << 8 | next()?,
                        )
                    }
                    n => (n + 1, (first & 15) << 8 | next()?),
                }
            };
            if back >= out.len() {
                return None;
            }
            for _ in 0..count {
                if out.len() >= len {
                    // The older format must not run past its end.
                    if kind == 0x10 {
                        return None;
                    }
                    break;
                }
                out.push(out[out.len() - back - 1]);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_formats_unpack() {
        // "abc", then six more bytes copied from three back.
        let old = [0x10, 9, 0, 0, 0b0001_0000, b'a', b'b', b'c', 0x30, 0x02];
        assert_eq!(decode(&old).unwrap(), b"abcabcabc");
        let new = [0x11, 9, 0, 0, 0b0001_0000, b'a', b'b', b'c', 0x50, 0x02];
        assert_eq!(decode(&new).unwrap(), b"abcabcabc");
        // The long forms of the newer format.
        let mut long = vec![0x11, 0x14, 0, 0, 0b0100_0000, b'x', 0x00, 0x20, 0x00];
        assert_eq!(decode(&long).unwrap(), vec![b'x'; 0x14]);
        long = vec![
            0x11,
            0x13,
            0x01,
            0,
            0b0100_0000,
            b'x',
            0x10,
            0x00,
            0x10,
            0x00,
        ];
        assert_eq!(decode(&long).unwrap(), vec![b'x'; 0x113]);
    }

    #[test]
    fn broken_data_is_refused() {
        assert_eq!(decode(&[0x12, 1, 0, 0, 0, 1]), None);
        assert_eq!(decode(&[0x10, 4, 0, 0, 0, 1]), None);
        // Reaches back before the start.
        assert_eq!(decode(&[0x11, 4, 0, 0, 0b1000_0000, 0x30, 0x00]), None);
    }
}

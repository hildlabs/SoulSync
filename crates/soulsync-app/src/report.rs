//! "Report a problem": puts what helps to find a bug into one zip file the
//! player can send along, so that nobody has to hunt for log files.
//!
//! Nothing is sent anywhere. The file lands next to the settings and its
//! folder opens, so the player sees what it holds.

use std::path::{Path, PathBuf};

/// Files larger than this are left out; a RAM dump is 4 MB, a save 512 KB.
const MAX_FILE: u64 = 16 << 20;
/// Reports kept; older ones are removed when a new one is made.
const KEEP: usize = 5;

/// Writes a report with `info` and the files that exist among `files`,
/// each under its name. Returns where it went.
pub fn write(
    dir: &Path,
    info: &str,
    files: &[(&str, PathBuf)],
    now: u64,
) -> std::io::Result<PathBuf> {
    let mut entries: Vec<(String, Vec<u8>)> = vec![("info.txt".into(), info.as_bytes().to_vec())];
    for (name, path) in files {
        let small = std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= MAX_FILE);
        if small {
            if let Ok(data) = std::fs::read(path) {
                entries.push(((*name).to_owned(), data));
            }
        }
    }
    let dir = dir.join("reports");
    std::fs::create_dir_all(&dir)?;
    tidy(&dir);
    let path = dir.join(format!("soulsync-report-{now}.zip"));
    std::fs::write(&path, zip(&entries))?;
    Ok(path)
}

/// Removes the oldest reports so that a new one makes no more than `KEEP`.
fn tidy(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut old: Vec<PathBuf> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("soulsync-report-") && n.ends_with(".zip"))
        })
        .collect();
    old.sort();
    let extra = (old.len() + 1).saturating_sub(KEEP);
    for p in old.into_iter().take(extra) {
        let _ = std::fs::remove_file(p);
    }
}

/// CRC-32 as zip files use it.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A zip file holding `entries` as they are, without compression: what it
/// holds is small, and so the format stays simple.
fn zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    // 1 January 1980, the earliest date a zip file knows; the file name of
    // the report says when it was made.
    const TIME: u16 = 0;
    const DATE: u16 = 0x21;
    // The names are UTF-8.
    const FLAGS: u16 = 0x0800;
    let mut out = Vec::new();
    let mut central = Vec::new();
    let u16le = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let u32le = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    for (name, data) in entries {
        let (crc, len, at) = (crc32(data), data.len() as u32, out.len() as u32);
        u32le(&mut out, 0x0403_4B50);
        for x in [20, FLAGS, 0, TIME, DATE] {
            u16le(&mut out, x);
        }
        for x in [crc, len, len] {
            u32le(&mut out, x);
        }
        u16le(&mut out, name.len() as u16);
        u16le(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        u32le(&mut central, 0x0201_4B50);
        for x in [20, 20, FLAGS, 0, TIME, DATE] {
            u16le(&mut central, x);
        }
        for x in [crc, len, len] {
            u32le(&mut central, x);
        }
        for x in [name.len() as u16, 0, 0, 0, 0] {
            u16le(&mut central, x);
        }
        u32le(&mut central, 0);
        u32le(&mut central, at);
        central.extend_from_slice(name.as_bytes());
    }
    let (at, size, n) = (out.len() as u32, central.len() as u32, entries.len() as u16);
    out.extend_from_slice(&central);
    u32le(&mut out, 0x0605_4B50);
    for x in [0, 0, n, n] {
        u16le(&mut out, x);
    }
    u32le(&mut out, size);
    u32le(&mut out, at);
    u16le(&mut out, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_is_the_zip_one() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn writes_a_zip_with_the_files_there_are() {
        let dir = std::env::temp_dir().join(format!("soulsync-report-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("live-log.txt");
        std::fs::write(&log, "a line\n").unwrap();
        let files = [
            ("live-log.txt", log),
            ("ram-dump.bin", dir.join("missing.bin")),
        ];
        for now in 1..=7 {
            write(&dir, "build 1", &files, now).unwrap();
        }
        let path = write(&dir, "build 1", &files, 8).unwrap();
        let data = std::fs::read(&path).unwrap();
        assert!(data.starts_with(&[0x50, 0x4B, 3, 4]));
        // info.txt and the log, the missing dump left out.
        let end = &data[data.len() - 22..];
        assert_eq!(u16::from_le_bytes([end[10], end[11]]), 2);
        assert!(data.windows(6).any(|w| w == b"a line"));
        let kept = std::fs::read_dir(dir.join("reports")).unwrap().count();
        assert_eq!(kept, KEEP);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

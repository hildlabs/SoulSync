//! The programs a randomized game is made with, fetched once on first use:
//! Universal Pokémon Randomizer ZX (GPL-3.0, like SoulSync) and, where no
//! Java is installed, an Eclipse Temurin Java runtime (GPL-2.0 with the
//! Classpath Exception). Both come straight from their projects' releases
//! on GitHub, are checked against a fixed SHA-256 and kept with the app's
//! data. A player who already has either can still choose their own.

use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// A program fetched as a zip archive.
pub struct Tool {
    /// Shown while it downloads.
    pub name: &'static str,
    url: &'static str,
    sha256: &'static str,
    /// Its folder under the app's data.
    dir: &'static str,
    /// The file that is run, inside that folder.
    file: &'static str,
}

pub const RANDOMIZER: Tool = Tool {
    name: "Universal Pokémon Randomizer ZX 4.6.1",
    url: "https://github.com/Ajarmar/universal-pokemon-randomizer-zx/releases/download/v4.6.1/PokeRandoZX-v4_6_1.zip",
    sha256: "26051fe8a6665ea0582dfcd4d1c3e4889da42a2f731f5fc530fff0cb691a43fd",
    dir: "randomizer-4.6.1",
    file: "PokeRandoZX.jar",
};

/// Only for Windows, the platform SoulSync ships for.
pub const JAVA: Tool = Tool {
    name: "Java (Eclipse Temurin 21)",
    url: "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.5%2B11/OpenJDK21U-jre_x64_windows_hotspot_21.0.5_11.zip",
    sha256: "1749b36cfac273cee11802bf3e90caada5062de6a3fef1a3814c0568b25fd654",
    dir: "java-21.0.5",
    file: "jdk-21.0.5+11-jre/bin/java.exe",
};

fn tools_dir() -> Option<PathBuf> {
    crate::local::data_dir().map(|d| d.join("tools"))
}

impl Tool {
    /// The program, if it was fetched before.
    pub fn installed(&self) -> Option<PathBuf> {
        let path = tools_dir()?.join(self.dir).join(self.file);
        path.is_file().then_some(path)
    }

    /// Downloads, checks and unpacks it; `progress` hears bytes done and
    /// the total, when known.
    pub fn install(&self, progress: &dyn Fn(u64, u64)) -> Result<PathBuf, String> {
        if let Some(path) = self.installed() {
            return Ok(path);
        }
        let root = tools_dir().ok_or("no folder for the app's data")?;
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let archive = root.join(format!("{}.zip", self.dir));
        download(self.url, &archive, self.sha256, progress)?;
        let unpacked = unpack(&archive, &root.join(format!("{}.part", self.dir)));
        let _ = std::fs::remove_file(&archive);
        let part = unpacked?;
        let dir = root.join(self.dir);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::rename(&part, &dir).map_err(|e| e.to_string())?;
        self.installed()
            .ok_or_else(|| format!("{} is not in the download", self.file))
    }
}

fn download(url: &str, to: &Path, sha256: &str, progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    let mut response = ureq::get(url)
        .header("User-Agent", "SoulSync")
        .call()
        .map_err(|e| e.to_string())?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = response.body_mut().as_reader();
    let mut file = std::fs::File::create(to).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    loop {
        let n = body.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        progress(done, total);
    }
    file.flush().map_err(|e| e.to_string())?;
    let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if got != sha256 {
        let _ = std::fs::remove_file(to);
        return Err(format!("the download of {url} is not the expected file"));
    }
    Ok(())
}

/// Unpacks `archive` into `into`, which it makes anew.
fn unpack(archive: &Path, into: &Path) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(into);
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        // Names that would leave the folder are skipped.
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let path = into.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(into.to_path_buf())
}

/// Whether `java` on the PATH starts.
fn system_java() -> bool {
    let mut command = std::process::Command::new("java");
    command.arg("-version");
    crate::fresh::quiet(&mut command)
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The Java to run the randomizer with: one fetched before, the installed
/// one, or (on Windows) one fetched now.
pub fn java(progress: &dyn Fn(&'static str, u64, u64)) -> Result<OsString, String> {
    if let Some(path) = JAVA.installed() {
        return Ok(path.into());
    }
    if system_java() {
        return Ok("java".into());
    }
    if !cfg!(windows) {
        return Err("Java is not installed".into());
    }
    JAVA.install(&|done, total| progress(JAVA.name, done, total))
        .map(Into::into)
}

/// The randomizer: the player's own if they chose one, else the one
/// fetched before or now.
pub fn randomizer(
    own: Option<&Path>,
    progress: &dyn Fn(&'static str, u64, u64),
) -> Result<PathBuf, String> {
    if let Some(own) = own.filter(|p| p.is_file()) {
        return Ok(own.to_path_buf());
    }
    RANDOMIZER.install(&|done, total| progress(RANDOMIZER.name, done, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downloads_only_the_expected_file() {
        use std::io::BufRead;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/a.zip", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut stream = stream.unwrap();
                let mut line = String::new();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                while reader.read_line(&mut line).unwrap() > 2 {
                    line.clear();
                }
                let body = b"hello zip";
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(body).unwrap();
            }
        });
        let dir = std::env::temp_dir().join(format!("soulsync-dl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let to = dir.join("a.zip");
        let sum = "c79b7ad0f6b0d9c5a32fdbc6a0f3e0f1f5ef2c1d84b7d9d1e4d58dba4c4fb4a6";
        let heard = std::cell::Cell::new(0);
        let got = download(&url, &to, sum, &|done, total| {
            assert_eq!(total, 9);
            heard.set(done);
        });
        assert!(got.is_err(), "a wrong checksum is refused");
        assert!(!to.exists());
        assert_eq!(heard.get(), 9);
        let right: String = Sha256::digest(b"hello zip")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        download(&url, &to, &right, &|_, _| {}).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), b"hello zip");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unpacks_inside_its_folder_only() {
        let dir = std::env::temp_dir().join(format!("soulsync-unpack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("a.zip");
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("jre/bin/java.exe", opts).unwrap();
            zip.write_all(b"java").unwrap();
            zip.start_file("../evil.txt", opts).unwrap();
            zip.write_all(b"no").unwrap();
            zip.finish().unwrap();
        }
        let out = unpack(&archive, &dir.join("out")).unwrap();
        assert_eq!(
            std::fs::read(out.join("jre/bin/java.exe")).unwrap(),
            b"java"
        );
        assert!(!dir.join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

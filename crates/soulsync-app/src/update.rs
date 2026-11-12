//! Updates from the project's releases on GitHub.
//!
//! Every build of the main branch is published as a release tagged
//! `build-<number>` with `SoulSync.exe` attached, and the app knows its own
//! build number. A newer release is offered in the app; installing it swaps
//! the program file and starts the new one. Nothing about the player is sent:
//! the app only reads the public list of releases.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// The newest releases. Not `releases/latest`: GitHub picks that by the date
/// of the tagged commit, so a build published late (a rerun) can hide newer
/// ones. The highest build number decides instead.
const RELEASES: &str = "https://api.github.com/repos/hildlabs/SoulSync/releases?per_page=20";
/// Where a release's files come from. Anything else is not installed.
const DOWNLOADS: &str = "https://github.com/hildlabs/SoulSync/releases/download/";
const ASSET: &str = "SoulSync.exe";
const AGENT: &str = "SoulSync";
/// Passed to the new program, which then waits for this one to quit.
pub const UPDATED_FLAG: &str = "--updated";

/// The number of the build this program comes from. Builds made by hand have
/// none and are never offered an update.
pub fn build() -> Option<u64> {
    option_env!("SOULSYNC_BUILD")?.parse().ok()
}

/// A published build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Release {
    pub build: u64,
    #[serde(skip)]
    url: String,
}

fn parse_release(json: &serde_json::Value) -> Option<Release> {
    let build = json["tag_name"]
        .as_str()?
        .strip_prefix("build-")?
        .parse()
        .ok()?;
    let url = json["assets"]
        .as_array()?
        .iter()
        .find(|a| a["name"] == ASSET)?["browser_download_url"]
        .as_str()?;
    url.starts_with(DOWNLOADS).then(|| Release {
        build,
        url: url.to_owned(),
    })
}

/// The release with the highest build number among `list`.
fn newest(list: &serde_json::Value) -> Option<Release> {
    list.as_array()?
        .iter()
        .filter(|r| r["draft"] != true && r["prerelease"] != true)
        .filter_map(parse_release)
        .max_by_key(|r| r.build)
}

/// The newest published build. Blocks.
pub fn latest() -> Option<Release> {
    let json: serde_json::Value = ureq::get(RELEASES)
        .header("User-Agent", AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    newest(&json)
}

/// The names a program file goes by while it is replaced.
fn beside(exe: &Path, tag: &str) -> PathBuf {
    let mut name = exe.file_stem().unwrap_or_default().to_os_string();
    name.push(format!(".{tag}"));
    if let Some(ext) = exe.extension() {
        name.push(".");
        name.push(ext);
    }
    exe.with_file_name(name)
}

/// Puts `new` in the place of `exe`. Windows does not let a running program
/// be overwritten, but it may be renamed, so the old one is kept beside it
/// until the next start.
fn swap(exe: &Path, new: &Path) -> std::io::Result<()> {
    let old = beside(exe, "old");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old)?;
    let moved = std::fs::rename(new, exe);
    if moved.is_err() {
        let _ = std::fs::rename(&old, exe);
    }
    moved
}

/// Removes what an update left behind.
pub fn tidy() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(beside(&exe, "old"));
        let _ = std::fs::remove_file(beside(&exe, "new"));
    }
}

/// How far an installation is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "step", rename_all = "lowercase")]
pub enum Progress {
    /// Downloading; the share that is there, where the size is known.
    Download {
        percent: Option<u8>,
    },
    /// Swapping the program file and starting the new one.
    Start,
    Failed,
}

/// Downloads a program to `to`, telling `on` how far it is. Blocks.
fn fetch(url: &str, to: &Path, on: &dyn Fn(Progress)) -> Result<(), String> {
    use std::io::{Read, Write};
    let mut response = ureq::get(url)
        .header("User-Agent", AGENT)
        .call()
        .map_err(|e| e.to_string())?;
    let size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|&n| n > 0);
    let mut file = std::fs::File::create(to).map_err(|e| e.to_string())?;
    let mut reader = response.body_mut().as_reader();
    let mut buffer = vec![0u8; 64 * 1024];
    let (mut done, mut told) = (0u64, None);
    loop {
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        let percent = size.map(|size| (done * 100 / size).min(100) as u8);
        if percent != told {
            told = percent;
            on(Progress::Download { percent });
        }
    }
    drop(file);
    // A page of text instead of a program must not replace the app.
    if crate::local::exe_machine(to).is_none() {
        let _ = std::fs::remove_file(to);
        return Err("the download is not a program".into());
    }
    Ok(())
}

/// Downloads a release, puts it in this program's place and starts it,
/// telling `on` how far it is. The caller quits afterwards. Blocks.
pub fn install(release: &Release, on: &dyn Fn(Progress)) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let new = beside(&exe, "new");
    on(Progress::Download { percent: None });
    fetch(&release.url, &new, on)?;
    on(Progress::Start);
    swap(&exe, &new).map_err(|e| e.to_string())?;
    std::process::Command::new(&exe)
        .arg(UPDATED_FLAG)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_release() {
        let json = serde_json::json!({
            "tag_name": "build-42",
            "assets": [
                { "name": "notes.txt", "browser_download_url": "https://github.com/hildlabs/SoulSync/releases/download/build-42/notes.txt" },
                { "name": "SoulSync.exe", "browser_download_url": "https://github.com/hildlabs/SoulSync/releases/download/build-42/SoulSync.exe" },
            ],
        });
        let release = parse_release(&json).unwrap();
        assert_eq!(release.build, 42);
        assert!(release.url.ends_with("/build-42/SoulSync.exe"));
        // Other tags, a missing program or a download from elsewhere: no update.
        let mut other = json.clone();
        other["tag_name"] = "v1.0".into();
        assert_eq!(parse_release(&other), None);
        let mut empty = json.clone();
        empty["assets"] = serde_json::json!([]);
        assert_eq!(parse_release(&empty), None);
        let mut elsewhere = json;
        elsewhere["assets"][1]["browser_download_url"] = "https://example.com/SoulSync.exe".into();
        assert_eq!(parse_release(&elsewhere), None);
    }

    #[test]
    fn takes_the_highest_build_not_the_first() {
        let release = |n: u64| {
            serde_json::json!({
                "tag_name": format!("build-{n}"),
                "assets": [{ "name": "SoulSync.exe", "browser_download_url": format!("https://github.com/hildlabs/SoulSync/releases/download/build-{n}/SoulSync.exe") }],
            })
        };
        // As GitHub listed them on 2026-11-12: a rerun of 56 published after 62.
        let mut draft = release(70);
        draft["draft"] = true.into();
        let list = serde_json::json!([release(56), release(62), release(54), draft]);
        assert_eq!(newest(&list).unwrap().build, 62);
        assert_eq!(newest(&serde_json::json!([])), None);
    }

    /// Answers one request with `body`.
    fn serve(body: Vec<u8>) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/SoulSync.exe", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
        });
        url
    }

    #[test]
    fn downloads_only_programs() {
        let dir = std::env::temp_dir().join(format!("soulsync-fetch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let to = dir.join("SoulSync.new.exe");
        let mut program = vec![0u8; 0x90];
        program[0x3C] = 0x80;
        program[0x80..0x86].copy_from_slice(b"PE\0\0\x64\x86");
        let steps = std::sync::Mutex::new(Vec::new());
        fetch(&serve(program.clone()), &to, &|p| {
            steps.lock().unwrap().push(p)
        })
        .unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), program);
        let last = steps.lock().unwrap().last().cloned();
        assert_eq!(last, Some(Progress::Download { percent: Some(100) }));
        assert!(fetch(&serve(b"<html>Not found</html>".to_vec()), &to, &|_| {}).is_err());
        assert!(!to.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn swaps_the_program_file() {
        let dir = std::env::temp_dir().join(format!("soulsync-swap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("SoulSync.exe");
        let new = beside(&exe, "new");
        assert_eq!(new, dir.join("SoulSync.new.exe"));
        std::fs::write(&exe, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        // Left over from an earlier update.
        std::fs::write(beside(&exe, "old"), b"older").unwrap();
        swap(&exe, &new).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert_eq!(std::fs::read(beside(&exe, "old")).unwrap(), b"old");
        assert!(!new.exists());
        // Without a download the program stays as it is.
        assert!(swap(&exe, &new).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

//! Which game a save or ROM belongs to.

use crate::lang::{Lang, Names};
use serde::{Deserialize, Serialize};

/// Games SoulSync can identify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Game {
    Black,
    White,
    Black2,
    White2,
}

impl Game {
    /// The version byte stored in generation 5 trainer data.
    pub fn from_save_version(v: u8) -> Option<Game> {
        match v {
            20 => Some(Game::White),
            21 => Some(Game::Black),
            22 => Some(Game::White2),
            23 => Some(Game::Black2),
            _ => None,
        }
    }

    /// The first three letters of a cartridge game code such as `IRED`.
    pub fn from_game_code(code: &str) -> Option<Game> {
        match code.get(..3)? {
            "IRB" => Some(Game::Black),
            "IRA" => Some(Game::White),
            "IRE" => Some(Game::Black2),
            "IRD" => Some(Game::White2),
            _ => None,
        }
    }

    pub fn generation(self) -> u8 {
        5
    }

    pub fn name(self) -> Names {
        let (en, de) = match self {
            Game::Black => ("Black", "Schwarz"),
            Game::White => ("White", "Weiß"),
            Game::Black2 => ("Black 2", "Schwarz 2"),
            Game::White2 => ("White 2", "Weiß 2"),
        };
        Names {
            en: en.to_owned(),
            de: de.to_owned(),
        }
    }

    pub fn label(self, lang: Lang) -> String {
        self.name().get(lang).to_owned()
    }
}

/// What the header of a Nintendo DS ROM says about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomInfo {
    /// Internal title, for example `POKEMON B2`.
    pub title: String,
    /// Four-letter game code, for example `IRED`.
    pub code: String,
    /// `None` for ROMs SoulSync does not know.
    pub game: Option<Game>,
}

/// Bytes of a ROM that [`read_rom_header`] needs.
pub const ROM_HEADER_LEN: usize = 16;

/// Identifies a ROM from its first [`ROM_HEADER_LEN`] bytes. Nothing else of
/// the ROM is read. Returns `None` if this does not look like a DS ROM header.
pub fn read_rom_header(header: &[u8]) -> Option<RomInfo> {
    let h = header.get(..ROM_HEADER_LEN)?;
    let text = |b: &[u8]| -> Option<String> {
        let b = &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())];
        b.iter()
            .all(|c| c.is_ascii_graphic() || *c == b' ')
            .then(|| String::from_utf8_lossy(b).trim().to_owned())
    };
    let code = text(&h[12..16])?;
    if code.len() != 4 {
        return None;
    }
    Some(RomInfo {
        title: text(&h[..12])?,
        game: Game::from_game_code(&code),
        code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_games() {
        assert_eq!(Game::from_save_version(23), Some(Game::Black2));
        assert_eq!(Game::from_save_version(0), None);
        let mut h = [0u8; 16];
        h[..10].copy_from_slice(b"POKEMON B2");
        h[12..].copy_from_slice(b"IRED");
        let info = read_rom_header(&h).unwrap();
        assert_eq!(info.title, "POKEMON B2");
        assert_eq!(info.game, Some(Game::Black2));
        assert_eq!(info.game.unwrap().label(Lang::De), "Schwarz 2");
        assert_eq!(read_rom_header(&[0xFF; 16]), None);
        assert_eq!(read_rom_header(&[0; 4]), None);
    }
}

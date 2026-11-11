//! User-facing CLI text in English and German.

use soulsync_core::Lang;

pub struct Text {
    pub usage: &'static str,
    pub trainer: &'static str,
    pub badges: &'static str,
    pub party: &'static str,
    pub boxes: &'static str,
    pub slot_party: &'static str,
    pub slot_box: &'static str,
    pub level: &'static str,
    pub hp: &'static str,
    pub met: &'static str,
    pub location: &'static str,
    pub egg: &'static str,
    pub error: &'static str,
    pub watching: &'static str,
    pub ev_caught: &'static str,
    pub ev_fainted: &'static str,
    pub ev_evolved: &'static str,
    pub ev_level: &'static str,
    pub ev_to_party: &'static str,
    pub ev_to_box: &'static str,
    pub ev_gone: &'static str,
    pub ev_badges: &'static str,
}

const EN: Text = Text {
    usage: "\
SoulSync: track Pokémon Soul Link runs live

Usage:
  soulsync inspect <save> [--json] [--lang en|de]   Show party, boxes and badges
  soulsync watch   <save> [--json] [--lang en|de]   Watch the save and report changes

<save> is the .dsv from DeSmuME's Battery folder, or a .sav file.
Supported: Pokémon Black 2 / White 2.
Language: --lang, or the SOULSYNC_LANG / LANG environment variables.",
    trainer: "Trainer",
    badges: "badges",
    party: "Party",
    boxes: "Boxes",
    slot_party: "Party",
    slot_box: "Box",
    level: "Lv",
    hp: "HP",
    met: "met at",
    location: "location",
    egg: "egg",
    error: "Error",
    watching: "Watching {path} ({party} in party, {boxes} in boxes). Save in-game to see changes. Ctrl+C to stop.",
    ev_caught: "New",
    ev_fainted: "Fainted (0 HP)",
    ev_evolved: "Evolved",
    ev_level: "is now Lv",
    ev_to_party: "joined the party",
    ev_to_box: "went to box",
    ev_gone: "No longer in the save",
    ev_badges: "Badges",
};

const DE: Text = Text {
    usage: "\
SoulSync: Soul-Link-Runs live verfolgen

Benutzung:
  soulsync inspect <spielstand> [--json] [--lang en|de]   Team, Boxen und Orden anzeigen
  soulsync watch   <spielstand> [--json] [--lang en|de]   Spielstand beobachten und Änderungen melden

<spielstand> ist die .dsv aus dem Battery-Ordner von DeSmuME oder eine .sav.
Unterstützt: Pokémon Schwarz 2 / Weiß 2.
Sprache: --lang oder die Umgebungsvariablen SOULSYNC_LANG / LANG.",
    trainer: "Trainer",
    badges: "Orden",
    party: "Team",
    boxes: "Boxen",
    slot_party: "Team",
    slot_box: "Box",
    level: "Lv",
    hp: "KP",
    met: "gefangen in",
    location: "Ort",
    egg: "Ei",
    error: "Fehler",
    watching: "Beobachte {path} ({party} im Team, {boxes} in Boxen). Speichere im Spiel, um Änderungen zu sehen. Strg+C beendet.",
    ev_caught: "Neu",
    ev_fainted: "Besiegt (0 KP)",
    ev_evolved: "Entwicklung",
    ev_level: "ist jetzt Lv",
    ev_to_party: "ist jetzt im Team",
    ev_to_box: "liegt jetzt in Box",
    ev_gone: "Nicht mehr im Spielstand",
    ev_badges: "Orden",
};

pub fn text(lang: Lang) -> &'static Text {
    match lang {
        Lang::En => &EN,
        Lang::De => &DE,
    }
}

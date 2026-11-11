//! Display languages.

use serde::Serialize;
use std::str::FromStr;

/// Languages SoulSync can display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    De,
}

impl Lang {
    /// Picks a language from a locale string such as `de_DE.UTF-8`.
    /// Anything that is not German falls back to English.
    pub fn from_locale(locale: &str) -> Lang {
        if locale.to_ascii_lowercase().starts_with("de") {
            Lang::De
        } else {
            Lang::En
        }
    }
}

impl FromStr for Lang {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "en" | "english" => Ok(Lang::En),
            "de" | "german" | "deutsch" => Ok(Lang::De),
            other => Err(format!("unknown language '{other}' (use en or de)")),
        }
    }
}

/// A name in every supported language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Names {
    pub en: String,
    pub de: String,
}

impl Names {
    pub fn get(&self, lang: Lang) -> &str {
        match lang {
            Lang::En => &self.en,
            Lang::De => &self.de,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_languages() {
        assert_eq!("DE".parse(), Ok(Lang::De));
        assert_eq!("en".parse(), Ok(Lang::En));
        assert!("fr".parse::<Lang>().is_err());
        assert_eq!(Lang::from_locale("de_AT.UTF-8"), Lang::De);
        assert_eq!(Lang::from_locale("C"), Lang::En);
    }
}

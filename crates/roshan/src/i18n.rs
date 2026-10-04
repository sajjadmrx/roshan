//! Translations. Each language is one TOML file in `locales/`, embedded at
//! build time. Adding a language means adding a file and one entry to
//! [`LANGUAGES`]; missing keys fall back to English.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;

use gpui_kit::{App, Global, SharedString};

pub struct Language {
    pub code: &'static str,
    /// The language's name in itself, shown in the picker.
    pub native_name: &'static str,
    pub rtl: bool,
    /// Native digits, if the language does not use 0-9.
    pub digits: Option<[char; 10]>,
    source: &'static str,
}

pub const LANGUAGES: &[Language] = &[
    Language {
        code: "en",
        native_name: "English",
        rtl: false,
        digits: None,
        source: include_str!("../locales/en.toml"),
    },
    Language {
        code: "fa",
        native_name: "فارسی",
        rtl: true,
        digits: Some(['۰', '۱', '۲', '۳', '۴', '۵', '۶', '۷', '۸', '۹']),
        source: include_str!("../locales/fa.toml"),
    },
];

pub fn language(code: &str) -> &'static Language {
    LANGUAGES
        .iter()
        .find(|l| l.code == code)
        .unwrap_or(&LANGUAGES[0])
}

/// Cheap to clone: the tables are shared.
#[derive(Clone)]
pub struct I18n {
    pub lang: &'static Language,
    strings: Arc<HashMap<String, String>>,
    fallback: Arc<HashMap<String, String>>,
}

impl Global for I18n {}

impl I18n {
    pub fn new(code: &str) -> Self {
        let lang = language(code);
        Self {
            lang,
            strings: Arc::new(flatten(lang.source)),
            fallback: Arc::new(flatten(LANGUAGES[0].source)),
        }
    }

    pub fn rtl(&self) -> bool {
        self.lang.rtl
    }

    pub fn t(&self, key: &str) -> SharedString {
        self.directional(self.raw(key)).into()
    }

    /// For right-to-left languages, embeds every line in an explicit RTL
    /// run (RLE…PDF). The text engine lays paragraphs out left to right, so
    /// without this, trailing punctuation and embedded Latin words land on
    /// the wrong side. Long right-to-left strings carry explicit line breaks
    /// in their locale file instead of relying on automatic wrapping.
    pub fn directional(&self, text: &str) -> String {
        if !self.lang.rtl {
            return text.to_owned();
        }
        text.split('\n')
            .map(|line| format!("\u{202B}{line}\u{202C}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn raw<'a>(&'a self, key: &'a str) -> &'a str {
        self.strings
            .get(key)
            .or_else(|| self.fallback.get(key))
            .map_or(key, String::as_str)
    }

    /// Translates `key` and fills `{name}` placeholders.
    pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> SharedString {
        let mut text = self.raw(key).to_owned();
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        self.directional(&text).into()
    }

    /// Formats a number with the language's digits.
    pub fn num(&self, n: impl Display) -> String {
        let text = n.to_string();
        match self.lang.digits {
            Some(digits) => text
                .chars()
                .map(|c| c.to_digit(10).map_or(c, |d| digits[d as usize]))
                .collect(),
            None => text,
        }
    }

    /// Joins already-translated fragments into one line, keeping the reading
    /// direction of the whole line. Right-to-left languages use the Persian
    /// comma: a middle dot is indistinguishable from the Persian zero (۰).
    pub fn join(&self, parts: &[SharedString]) -> SharedString {
        let plain: Vec<String> = parts
            .iter()
            .map(|p| p.replace(['\u{202B}', '\u{202C}'], ""))
            .collect();
        let separator = if self.lang.rtl { "، " } else { " · " };
        self.directional(&plain.join(separator)).into()
    }

    pub fn items(&self, n: usize) -> SharedString {
        if n == 1 {
            self.t("sessions.items_one")
        } else {
            self.tf("sessions.items_other", &[("n", &self.num(n))])
        }
    }

    pub fn seconds(&self, n: u32) -> SharedString {
        self.tf("editor.seconds", &[("n", &self.num(n))])
    }
}

/// Turns nested TOML tables into dotted keys: `[run] skip = ".."` -> `run.skip`.
fn flatten(source: &str) -> HashMap<String, String> {
    fn walk(prefix: &str, table: &toml::Table, out: &mut HashMap<String, String>) {
        for (key, value) in table {
            let full = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            match value {
                toml::Value::String(s) => {
                    out.insert(full, s.clone());
                }
                toml::Value::Table(t) => walk(&full, t, out),
                _ => {}
            }
        }
    }
    let mut out = HashMap::new();
    if let Ok(table) = source.parse::<toml::Table>() {
        walk("", &table, &mut out);
    }
    out
}

pub fn i18n(cx: &App) -> I18n {
    cx.global::<I18n>().clone()
}

pub fn t(cx: &App, key: &str) -> SharedString {
    cx.global::<I18n>().t(key)
}

/// Picks the best initial language for a first run.
pub fn guess_language() -> &'static str {
    roshan_platform::system_language()
        .and_then(|code| LANGUAGES.iter().find(|l| l.code == code))
        .map_or("en", |l| l.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_parses_and_matches_english_keys() {
        let english = flatten(LANGUAGES[0].source);
        assert!(english.len() > 50);
        for lang in LANGUAGES {
            let strings = flatten(lang.source);
            let mut missing: Vec<_> = english
                .keys()
                .filter(|k| !strings.contains_key(*k))
                .collect();
            let mut extra: Vec<_> = strings
                .keys()
                .filter(|k| !english.contains_key(*k))
                .collect();
            missing.sort();
            extra.sort();
            assert!(missing.is_empty(), "{} is missing {missing:?}", lang.code);
            assert!(extra.is_empty(), "{} has unknown keys {extra:?}", lang.code);
        }
    }

    /// TYPOGRAPHY.md: no tanween, no trailing periods, no exclamation marks,
    /// casual words over formal ones, and half-spaces where they belong.
    #[test]
    fn persian_copy_follows_the_typography_guide() {
        let strings = flatten(language("fa").source);
        let mut problems = Vec::new();
        for (key, value) in &strings {
            let mut complain = |why: &str| problems.push(format!("{key}: {why}: {value}"));
            if value.contains('\u{064B}') {
                complain("tanween");
            }
            if value.contains('!') {
                complain("exclamation mark");
            }
            if value.contains('·') {
                complain("middle dot reads as the Persian zero, use «،»");
            }
            if value
                .lines()
                .any(|line| line.ends_with('.') || line.ends_with('۔'))
            {
                complain("trailing period");
            }
            let words: Vec<&str> = value.split_whitespace().collect();
            for formal in ["را", "انصراف", "جهت", "صرفا", "نمایید", "فرمایید"]
            {
                if words.contains(&formal) {
                    complain("formal wording");
                }
            }
            // "می" and plural "ها" attach with a half-space, never a space.
            if words
                .iter()
                .any(|w| *w == "می" || *w == "ها" || *w == "های")
            {
                complain("space instead of half-space");
            }
        }
        problems.sort();
        assert!(problems.is_empty(), "{problems:#?}");
    }

    #[test]
    fn placeholders_survive_translation() {
        let english = flatten(LANGUAGES[0].source);
        for lang in &LANGUAGES[1..] {
            let strings = flatten(lang.source);
            for (key, en) in &english {
                for part in en.split('{').skip(1) {
                    let name = part.split('}').next().unwrap();
                    assert!(
                        strings[key].contains(&format!("{{{name}}}")),
                        "{}: {key} lost {{{name}}}",
                        lang.code
                    );
                }
            }
        }
    }

    #[test]
    fn formats_numbers_and_placeholders() {
        let fa = I18n::new("fa");
        assert_eq!(fa.num(2048), "۲۰۴۸");
        assert_eq!(fa.items(4).as_ref(), "\u{202B}۴ مورد\u{202C}");
        assert_eq!(
            fa.directional("الف\nب"),
            "\u{202B}الف\u{202C}\n\u{202B}ب\u{202C}"
        );
        let en = I18n::new("en");
        assert_eq!(en.items(1).as_ref(), "1 item");
        assert_eq!(en.tf("run.next_in", &[("s", "3")]).as_ref(), "Next in 3s");
        // Unknown languages fall back to English.
        assert_eq!(I18n::new("xx").lang.code, "en");
    }
}

//! Parsing for freedesktop `.desktop` files and their `Exec=` lines.
//! Pure functions, so they are tested on every platform.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::collections::HashMap;

/// The `[Desktop Entry]` group of a `.desktop` file.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    pub fields: HashMap<String, String>,
}

impl DesktopEntry {
    pub fn parse(text: &str) -> Self {
        let mut fields = HashMap::new();
        let mut in_main = false;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') {
                in_main = line == "[Desktop Entry]";
                continue;
            }
            if !in_main {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                fields
                    .entry(key.trim().to_owned())
                    .or_insert_with(|| unescape(value.trim()));
            }
        }
        Self { fields }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    fn flag(&self, key: &str) -> bool {
        self.get(key)
            .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    }

    /// The best name for `locale` (e.g. `fa_IR.UTF-8`), per the spec's
    /// lookup order: lang_COUNTRY, then lang, then the plain key.
    pub fn localized(&self, key: &str, locale: Option<&str>) -> Option<&str> {
        if let Some(locale) = locale {
            let base = locale.split(['.', '@']).next().unwrap_or(locale);
            let lang = base.split('_').next().unwrap_or(base);
            for candidate in [base, lang] {
                if let Some(v) = self.get(&format!("{key}[{candidate}]")) {
                    return Some(v);
                }
            }
        }
        self.get(key)
    }

    /// Whether this entry is an application meant to be shown on `desktops`
    /// (the `XDG_CURRENT_DESKTOP` list).
    pub fn is_visible_app(&self, desktops: &[String]) -> bool {
        if self.get("Type") != Some("Application")
            || self.flag("NoDisplay")
            || self.flag("Hidden")
            || self.get("Exec").is_none_or(str::is_empty)
        {
            return false;
        }
        let listed = |key: &str| {
            self.get(key).map(|list| {
                list.split(';').any(|d| {
                    !d.is_empty() && desktops.iter().any(|cur| cur.eq_ignore_ascii_case(d))
                })
            })
        };
        if listed("OnlyShowIn") == Some(false) {
            return false;
        }
        listed("NotShowIn") != Some(true)
    }

    pub fn terminal(&self) -> bool {
        self.flag("Terminal")
    }
}

/// Resolves `\s`, `\n`, `\t`, `\r` and `\\` in a value string.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Splits an `Exec=` value into program and arguments, removing field codes
/// (no files or URLs are passed when launching from Roshan).
pub fn parse_exec(exec: &str, icon: Option<&str>, name: &str, file: &str) -> Vec<String> {
    let mut args = Vec::new();
    for token in tokenize(exec) {
        match token.as_str() {
            "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => {}
            // Flatpak's file-forwarding markers around the (absent) files.
            "@@" | "@@u" | "@@f" => {}
            "%i" => {
                if let Some(icon) = icon.filter(|i| !i.is_empty()) {
                    args.push("--icon".to_owned());
                    args.push(icon.to_owned());
                }
            }
            _ => {
                let expanded = token
                    .replace("%c", name)
                    .replace("%k", file)
                    .replace("%%", "%");
                args.push(expanded);
            }
        }
    }
    args
}

/// Tokenizes per the spec: whitespace separates arguments; double quotes
/// group, inside which `\"`, `` \` ``, `\$` and `\\` are escapes.
fn tokenize(exec: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quoted = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                in_token = true;
            }
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            c => {
                current.push(c);
                in_token = true;
            }
        }
    }
    if in_token {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
[Desktop Entry]
Type=Application
Name=Text Editor
Name[fa]=ویرایشگر متن
Exec=\"/opt/My Editor/editor\" --new-window %U
Icon=editor
Keywords=edit;text;

[Desktop Action new]
Name=Should be ignored
Exec=ignored
";

    #[test]
    fn parses_main_group_only() {
        let entry = DesktopEntry::parse(SAMPLE);
        assert_eq!(entry.get("Name"), Some("Text Editor"));
        assert_eq!(
            entry.get("Exec"),
            Some("\"/opt/My Editor/editor\" --new-window %U")
        );
        assert!(entry.is_visible_app(&[]));
    }

    #[test]
    fn localizes_names() {
        let entry = DesktopEntry::parse(SAMPLE);
        assert_eq!(
            entry.localized("Name", Some("fa_IR.UTF-8")),
            Some("ویرایشگر متن")
        );
        assert_eq!(entry.localized("Name", Some("de_DE")), Some("Text Editor"));
        assert_eq!(entry.localized("Name", None), Some("Text Editor"));
    }

    #[test]
    fn hides_entries_that_ask_to_be_hidden() {
        let hidden =
            DesktopEntry::parse("[Desktop Entry]\nType=Application\nExec=x\nNoDisplay=true\n");
        assert!(!hidden.is_visible_app(&[]));
        let link = DesktopEntry::parse("[Desktop Entry]\nType=Link\nURL=https://x\n");
        assert!(!link.is_visible_app(&[]));
        let kde_only =
            DesktopEntry::parse("[Desktop Entry]\nType=Application\nExec=x\nOnlyShowIn=KDE;\n");
        assert!(!kde_only.is_visible_app(&["GNOME".into()]));
        assert!(kde_only.is_visible_app(&["KDE".into()]));
        let not_gnome =
            DesktopEntry::parse("[Desktop Entry]\nType=Application\nExec=x\nNotShowIn=GNOME;\n");
        assert!(!not_gnome.is_visible_app(&["ubuntu".into(), "GNOME".into()]));
    }

    #[test]
    fn parses_exec_lines() {
        assert_eq!(
            parse_exec(
                "\"/opt/My Editor/editor\" --new-window %U",
                None,
                "Editor",
                "/a.desktop"
            ),
            ["/opt/My Editor/editor", "--new-window"]
        );
        assert_eq!(
            parse_exec(
                "app %i --name %c --file=%k 100%%",
                Some("app-icon"),
                "App",
                "/x.desktop"
            ),
            [
                "app",
                "--icon",
                "app-icon",
                "--name",
                "App",
                "--file=/x.desktop",
                "100%"
            ]
        );
        assert_eq!(
            parse_exec(r#"sh -c "echo \"hi\" \$HOME""#, None, "", ""),
            ["sh", "-c", r#"echo "hi" $HOME"#]
        );
        assert_eq!(
            parse_exec(
                "flatpak run --branch=stable org.example.App @@u %U @@",
                None,
                "",
                ""
            ),
            ["flatpak", "run", "--branch=stable", "org.example.App"]
        );
    }
}

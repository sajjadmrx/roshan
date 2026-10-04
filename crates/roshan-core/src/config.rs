//! The configuration file: a single human-readable TOML document.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{MAX_WAIT_SECS, Session};

/// Version of the on-disk format. Bump it (and add a migration) whenever a
/// change is not backwards compatible.
pub const CURRENT_SCHEMA: u32 = 1;

pub const FILE_NAME: &str = "roshan.toml";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub schema: u32,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default, rename = "session")]
    pub sessions: Vec<Session>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema: CURRENT_SCHEMA,
            settings: Settings::default(),
            sessions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// UI language code (e.g. "en", "fa"). `None` until the user picked one,
    /// which is how the first-run language prompt is detected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub theme: ThemePreference,
    /// Close Roshan once a run finished without failures.
    pub close_after_run: bool,
    /// Terminal used for command items on Linux, e.g. `"kitty -e"`.
    /// Ignored on Windows and macOS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read or write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not valid: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("{path} was written by a newer version of Roshan (schema {found})")]
    UnsupportedSchema { path: PathBuf, found: u32 },
    #[error("could not serialize configuration: {0}")]
    Serialize(#[from] toml::ser::Error),
}

impl Config {
    /// Loads the configuration. A missing file is not an error: it yields the
    /// empty default (a fresh installation has no sessions).
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(ConfigError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        Self::parse(&text, path)
    }

    pub fn parse(text: &str, path: &Path) -> Result<Self, ConfigError> {
        let mut config: Config = toml::from_str(text).map_err(|e| ConfigError::Parse {
            path: path.to_owned(),
            message: e.message().to_owned(),
        })?;
        if config.schema > CURRENT_SCHEMA {
            return Err(ConfigError::UnsupportedSchema {
                path: path.to_owned(),
                found: config.schema,
            });
        }
        config.normalize();
        Ok(config)
    }

    /// Clamps values a hand-edited file could get wrong.
    fn normalize(&mut self) {
        self.schema = CURRENT_SCHEMA;
        for session in &mut self.sessions {
            for item in &mut session.items {
                item.wait_after_secs = item.wait_after_secs.min(MAX_WAIT_SECS);
            }
        }
    }

    pub fn to_toml(&self) -> Result<String, ConfigError> {
        let body = toml::to_string_pretty(self)?;
        Ok(format!(
            "# Roshan sessions. This file is safe to edit by hand while Roshan is closed.\n\n{body}"
        ))
    }

    /// Writes atomically: a temporary file in the same directory is written,
    /// flushed and renamed over the old file, so a crash never leaves a
    /// half-written configuration behind.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let text = self.to_toml()?;
        let io = |source| ConfigError::Io {
            path: path.to_owned(),
            source,
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(io)?;
        }
        let tmp = path.with_extension("toml.tmp");
        {
            let mut file = fs::File::create(&tmp).map_err(io)?;
            restrict_permissions(&file);
            file.write_all(text.as_bytes()).map_err(io)?;
            file.sync_all().map_err(io)?;
        }
        fs::rename(&tmp, path).map_err(io)
    }

    /// Moves an unreadable configuration aside so it is never overwritten,
    /// returning where it went.
    pub fn back_up_broken(path: &Path) -> std::io::Result<PathBuf> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let backup = path.with_extension(format!("toml.broken-{stamp}"));
        fs::rename(path, &backup)?;
        Ok(backup)
    }
}

#[cfg(unix)]
fn restrict_permissions(file: &fs::File) {
    use std::os::unix::fs::PermissionsExt;
    // The file can contain commands; keep it private to the user.
    let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_: &fs::File) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AppTarget, ItemKind, LaunchItem};

    fn sample() -> Config {
        let mut session = Session::new("Test session");
        session.badge = Some("☀".into());
        let mut app = LaunchItem::new(
            "Some App",
            ItemKind::App {
                target: AppTarget::WindowsApp {
                    id: "Vendor.App_abc!App".into(),
                },
            },
        );
        app.wait_after_secs = 5;
        session.items.push(app);
        session.items.push(LaunchItem::new(
            "server",
            ItemKind::Command {
                line: "run-server --port 1".into(),
                cwd: Some("C:\\work".into()),
                terminal: true,
            },
        ));
        session.items.push(LaunchItem::new(
            "hidden",
            ItemKind::Command {
                line: "echo hi".into(),
                cwd: None,
                terminal: false,
            },
        ));
        session.items.push(LaunchItem::new(
            "docs",
            ItemKind::Open {
                target: "https://example.com".into(),
            },
        ));
        Config {
            schema: CURRENT_SCHEMA,
            settings: Settings {
                language: Some("fa".into()),
                theme: ThemePreference::Dark,
                close_after_run: true,
                terminal: None,
            },
            sessions: vec![session],
        }
    }

    #[test]
    fn round_trips_through_toml() {
        let config = sample();
        let text = config.to_toml().unwrap();
        let back = Config::parse(&text, Path::new("x")).unwrap();
        assert_eq!(config, back);
    }

    #[test]
    fn save_and_load_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join(FILE_NAME);
        let config = sample();
        config.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), config);
        // Saving again replaces the file in place.
        config.save(&path).unwrap();
        assert!(!path.with_extension("toml.tmp").exists());
    }

    #[test]
    fn missing_file_is_empty_default() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(&dir.path().join("none.toml")).unwrap();
        assert!(config.sessions.is_empty());
        assert_eq!(config.settings.language, None);
    }

    #[test]
    fn corrupt_file_is_an_error_not_a_default() {
        let err = Config::parse("schema = 1\n[[session]\n", Path::new("x")).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn newer_schema_is_rejected() {
        let err = Config::parse("schema = 99\n", Path::new("x")).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::UnsupportedSchema { found: 99, .. }
        ));
    }

    #[test]
    fn hand_written_file_is_accepted_and_clamped() {
        let text = r#"
schema = 1

[[session]]
name = "Hand written"

[[session.item]]
name = "server"
kind = "command"
line = "serve"
wait_after_secs = 99999

[[session.item]]
name = "site"
kind = "open"
target = "https://example.com"
"#;
        let config = Config::parse(text, Path::new("x")).unwrap();
        let items = &config.sessions[0].items;
        assert_eq!(items[0].wait_after_secs, MAX_WAIT_SECS);
        assert!(matches!(
            items[0].kind,
            ItemKind::Command { terminal: true, .. }
        ));
        assert_eq!(items[1].wait_after_secs, 0);
    }

    #[test]
    fn broken_file_is_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        fs::write(&path, "not toml [").unwrap();
        let backup = Config::back_up_broken(&path).unwrap();
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(backup).unwrap(), "not toml [");
    }
}

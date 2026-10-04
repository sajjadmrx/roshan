//! Everything Roshan needs from the operating system, behind one small API:
//!
//! * [`discover_apps`] lists applications that are really installed,
//! * [`app_icon`] returns an application's real icon (cached on disk),
//! * [`SystemLauncher`] launches items for the engine,
//! * [`config_path`] / [`system_language`] for app setup.
//!
//! Exactly one OS module is compiled. Each provides the same private
//! functions; this file is the only public surface.

use std::path::{Path, PathBuf};

use roshan_core::{AppTarget, ItemKind, LaunchError, Launched, Launcher};

mod icon_cache;
pub mod open_target;
pub mod update;

// `.desktop` parsing is pure code; it is compiled for tests everywhere.
#[cfg(any(target_os = "linux", test))]
mod desktop_entry;
#[cfg(unix)]
mod unix;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as os;
#[cfg(target_os = "macos")]
use macos as os;
#[cfg(windows)]
use windows as os;

/// Edge length, in physical pixels, of extracted icons. Large enough for a
/// 32px icon on a 200% display.
pub const ICON_SIZE: u32 = 64;

/// An application that exists on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredApp {
    pub name: String,
    pub target: AppTarget,
    /// Secondary text for the picker, such as the executable path.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconFormat {
    Png,
    Svg,
}

/// Encoded icon image, ready for the UI to decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    pub format: IconFormat,
    pub bytes: Vec<u8>,
}

/// Lists installed applications, sorted by name. This walks OS registries,
/// not the filesystem at large; call it off the UI thread.
pub fn discover_apps() -> Vec<DiscoveredApp> {
    let mut apps = os::discover_apps();
    let mut seen = std::collections::HashSet::new();
    apps.retain(|app| seen.insert(app.name.to_lowercase()));
    apps.sort_by_cached_key(|app| app.name.to_lowercase());
    apps
}

/// Returns the application's real icon, extracting it on a cache miss.
/// Call it off the UI thread.
pub fn app_icon(target: &AppTarget) -> Option<Icon> {
    let key = target.cache_key();
    if let Some(icon) = icon_cache::get(&key) {
        return Some(icon);
    }
    let icon = os::app_icon(target)?;
    icon_cache::put(&key, &icon);
    Some(icon)
}

/// Lets the user pick an executable that discovery did not list.
/// Turns a path chosen in a file dialog into a launch target.
pub fn target_for_path(path: &Path) -> AppTarget {
    os::target_for_path(path)
}

/// Display name for a hand-picked application path.
pub fn name_for_path(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Location of `roshan.toml`. `ROSHAN_CONFIG` overrides it, e.g. for a
/// portable setup or for testing.
pub fn config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("ROSHAN_CONFIG") {
        return PathBuf::from(path);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Roshan")
        .join(roshan_core::config::FILE_NAME)
}

/// Makes sure only one Roshan runs at a time, so two windows never overwrite
/// each other's changes. Returns `false` if another instance is already
/// running; with `focus_existing`, its window is brought to the front.
pub fn claim_single_instance(focus_existing: bool) -> bool {
    os::claim_single_instance(focus_existing)
}

/// After an update the new copy starts while the old one is still closing:
/// wait for it (up to `timeout`) instead of giving up straight away.
pub fn claim_single_instance_patiently(timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if os::claim_single_instance(false) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return os::claim_single_instance(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// Whether Roshan opens automatically when the user signs in. Read from the
/// OS every time, so turning it off elsewhere (Task Manager, System Settings)
/// is respected.
pub fn start_at_login() -> bool {
    os::start_at_login()
}

/// Adds Roshan to (or removes it from) the programs started at sign-in.
pub fn set_start_at_login(enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    os::set_start_at_login(enabled, &exe)
}

/// Keeps an enabled sign-in entry pointing at this executable, e.g. after the
/// user moved or updated Roshan. Does nothing when it is disabled.
pub fn refresh_start_at_login() {
    if os::start_at_login() {
        let _ = set_start_at_login(true);
    }
}

/// Command-line flag the sign-in entry passes, so a start at login can be told
/// apart from a manual start.
pub const STARTUP_FLAG: &str = "--startup";

/// The OS UI language as a lowercase ISO 639-1 code, if known.
pub fn system_language() -> Option<String> {
    os::system_language().map(|tag| {
        tag.split(['-', '_', '.'])
            .next()
            .unwrap_or_default()
            .to_lowercase()
    })
}

/// The real launcher used by the engine.
#[derive(Debug, Default, Clone)]
pub struct SystemLauncher {
    /// Linux only: user-configured terminal command, e.g. `"kitty -e"`.
    pub terminal: Option<String>,
}

impl Launcher for SystemLauncher {
    fn check(&self, kind: &ItemKind) -> Result<(), LaunchError> {
        match kind {
            ItemKind::App { target } => os::check_app(target),
            ItemKind::Command { cwd, .. } => match cwd {
                Some(dir) if !dir.is_dir() => Err(LaunchError::NotFound(dir.display().to_string())),
                _ => Ok(()),
            },
            ItemKind::Open { target } => open_target::classify(target).map(|_| ()),
        }
    }

    fn launch(&self, kind: &ItemKind) -> Result<Launched, LaunchError> {
        match kind {
            ItemKind::App { target } => os::launch_app(target),
            ItemKind::Command {
                line,
                cwd,
                terminal,
            } => {
                let cwd = cwd.clone().or_else(dirs::home_dir);
                os::run_command(line, cwd.as_deref(), *terminal, self.terminal.as_deref())
            }
            ItemKind::Open { target } => {
                let target = open_target::classify(target)?;
                os::open(&target)
            }
        }
    }
}

pub use open_target::OpenTarget;

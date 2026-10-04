//! Linux (freedesktop) integration.
//!
//! * Discovery reads `.desktop` entries from the XDG data directories, which
//!   include Flatpak and Snap exports; entries earlier in the search path
//!   override later ones with the same id, as the spec requires.
//! * Icons resolve through the icon theme (current theme, its parents, then
//!   `hicolor`) or `/usr/share/pixmaps`. PNG and SVG are supported.
//! * Apps launch by their parsed `Exec=` line, without a shell. Commands run
//!   in the user's login shell, in a terminal or in the background.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use roshan_core::{AppTarget, LaunchError, Launched};

use crate::desktop_entry::{DesktopEntry, parse_exec};
use crate::unix::{find_in_path, language_from_env, spawn_detached, user_shell};
use crate::{DiscoveredApp, ICON_SIZE, Icon, IconFormat, OpenTarget};

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// `$XDG_DATA_HOME` followed by `$XDG_DATA_DIRS`, plus the Flatpak and Snap
/// export directories when a distribution forgot to add them.
fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".local/share")),
    ];
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    dirs.extend(
        system
            .split(':')
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
    );
    for extra in [
        home().join(".local/share/flatpak/exports/share"),
        PathBuf::from("/var/lib/flatpak/exports/share"),
        PathBuf::from("/var/lib/snapd/desktop"),
    ] {
        if !dirs.contains(&extra) {
            dirs.push(extra);
        }
    }
    dirs
}

fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|d| !d.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn discover_apps() -> Vec<DiscoveredApp> {
    let desktops = current_desktops();
    let locale = language_from_env();
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut apps = Vec::new();
    for dir in data_dirs() {
        let root = dir.join("applications");
        let mut files = Vec::new();
        collect_desktop_files(&root, &root, &mut files);
        for (id, path) in files {
            // The first directory that defines an id wins, even if hidden.
            if seen.insert(id.clone(), ()).is_some() {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let entry = DesktopEntry::parse(&text);
            if !entry.is_visible_app(&desktops) {
                continue;
            }
            if let Some(try_exec) = entry.get("TryExec")
                && find_in_path(try_exec).is_none()
            {
                continue;
            }
            let Some(name) = entry.localized("Name", locale.as_deref()) else {
                continue;
            };
            apps.push(DiscoveredApp {
                name: name.to_owned(),
                detail: entry
                    .localized("Comment", locale.as_deref())
                    .map(str::to_owned),
                target: AppTarget::DesktopEntry { id, path },
            });
        }
    }
    apps
}

/// Desktop ids are paths relative to `applications/` with `/` turned into `-`.
fn collect_desktop_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_desktop_files(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "desktop") {
            let id = path
                .strip_prefix(root)
                .map(|rel| rel.to_string_lossy().replace('/', "-"))
                .unwrap_or_default();
            out.push((id, path));
        }
    }
}

fn load_entry(path: &Path) -> Result<DesktopEntry, LaunchError> {
    fs::read_to_string(path)
        .map(|text| DesktopEntry::parse(&text))
        .map_err(|_| LaunchError::NotFound(path.display().to_string()))
}

pub fn check_app(target: &AppTarget) -> Result<(), LaunchError> {
    match target {
        AppTarget::DesktopEntry { path, .. } | AppTarget::Executable { path } if path.exists() => {
            Ok(())
        }
        AppTarget::DesktopEntry { path, .. } | AppTarget::Executable { path } => {
            Err(LaunchError::NotFound(path.display().to_string()))
        }
        _ => Err(LaunchError::Unsupported),
    }
}

pub fn target_for_path(path: &Path) -> AppTarget {
    if path.extension().is_some_and(|e| e == "desktop") {
        AppTarget::DesktopEntry {
            id: crate::unix::file_name_lossy(path),
            path: path.to_owned(),
        }
    } else {
        AppTarget::Executable {
            path: path.to_owned(),
        }
    }
}

// ------------------------------------------------------------------- icons

pub fn app_icon(target: &AppTarget) -> Option<Icon> {
    let AppTarget::DesktopEntry { path, .. } = target else {
        return None;
    };
    let entry = load_entry(path).ok()?;
    let icon = entry.get("Icon")?;
    let file = if icon.starts_with('/') {
        Some(PathBuf::from(icon)).filter(|p| p.is_file())
    } else {
        lookup_icon(icon)
    }?;
    let bytes = fs::read(&file).ok()?;
    match file.extension().and_then(|e| e.to_str()) {
        Some("svg") => Some(Icon {
            format: IconFormat::Svg,
            bytes,
        }),
        Some("png") => {
            let image =
                image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?;
            crate::icon_cache::encode_png(
                image.width(),
                image.height(),
                image.to_rgba8().into_raw(),
            )
        }
        _ => None,
    }
}

fn icon_base_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![home().join(".icons")];
    dirs.extend(data_dirs().into_iter().map(|d| d.join("icons")));
    dirs
}

/// The icon theme the desktop is using, from GTK or KDE settings.
fn current_theme() -> Option<String> {
    let config = dirs::config_dir()?;
    let read_key = |file: PathBuf, key: &str| -> Option<String> {
        fs::read_to_string(file).ok()?.lines().find_map(|line| {
            let (k, v) = line.split_once('=')?;
            (k.trim() == key).then(|| v.trim().trim_matches('"').to_owned())
        })
    };
    read_key(config.join("gtk-4.0/settings.ini"), "gtk-icon-theme-name")
        .or_else(|| read_key(config.join("gtk-3.0/settings.ini"), "gtk-icon-theme-name"))
        .or_else(|| read_key(config.join("kdeglobals"), "Theme"))
        .filter(|t| !t.is_empty())
}

fn theme_parents(theme: &str, bases: &[PathBuf]) -> Vec<String> {
    bases
        .iter()
        .find_map(|base| fs::read_to_string(base.join(theme).join("index.theme")).ok())
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("Inherits=")
                    .map(|v| v.split(',').map(|s| s.trim().to_owned()).collect())
            })
        })
        .unwrap_or_default()
}

fn lookup_icon(name: &str) -> Option<PathBuf> {
    let bases = icon_base_dirs();
    let mut themes: Vec<String> = Vec::new();
    if let Some(theme) = current_theme() {
        themes.extend(theme_parents(&theme, &bases));
        themes.insert(0, theme);
    }
    for fallback in ["hicolor", "Adwaita", "breeze"] {
        if !themes.iter().any(|t| t == fallback) {
            themes.push(fallback.to_owned());
        }
    }
    let wanted = ICON_SIZE;
    let sizes = [
        format!("{wanted}x{wanted}"),
        "48x48".to_owned(),
        "96x96".to_owned(),
        "128x128".to_owned(),
        "256x256".to_owned(),
        "scalable".to_owned(),
        "32x32".to_owned(),
    ];
    for theme in &themes {
        for base in &bases {
            let root = base.join(theme);
            if !root.is_dir() {
                continue;
            }
            for size in &sizes {
                // Both "48x48/apps" (freedesktop) and "apps/48" (some themes).
                for dir in [
                    root.join(size).join("apps"),
                    root.join("apps")
                        .join(size.split('x').next().unwrap_or(size)),
                ] {
                    for ext in ["png", "svg"] {
                        let candidate = dir.join(format!("{name}.{ext}"));
                        if candidate.is_file() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
    }
    ["png", "svg"].iter().find_map(|ext| {
        let candidate = PathBuf::from(format!("/usr/share/pixmaps/{name}.{ext}"));
        candidate.is_file().then_some(candidate)
    })
}

// ---------------------------------------------------------------- launching

pub fn launch_app(target: &AppTarget) -> Result<Launched, LaunchError> {
    match target {
        AppTarget::DesktopEntry { path, .. } => {
            let entry = load_entry(path)?;
            let name = entry.get("Name").unwrap_or_default().to_owned();
            let argv = parse_exec(
                entry.get("Exec").unwrap_or_default(),
                entry.get("Icon"),
                &name,
                &path.to_string_lossy(),
            );
            let Some((program, args)) = argv.split_first() else {
                return Err(LaunchError::NotFound(path.display().to_string()));
            };
            if entry.terminal() {
                let mut line = crate::unix::sh_quote(program);
                for arg in args {
                    line.push(' ');
                    line.push_str(&crate::unix::sh_quote(arg));
                }
                let cwd = entry.get("Path").map(Path::new);
                return run_command(&line, cwd, true, None);
            }
            let mut cmd = Command::new(program);
            cmd.args(args);
            match entry.get("Path").filter(|p| !p.is_empty()) {
                Some(dir) => cmd.current_dir(dir),
                None => cmd.current_dir(home()),
            };
            spawn_detached(&mut cmd, program)
        }
        AppTarget::Executable { path } => {
            let mut cmd = Command::new(path);
            if let Some(dir) = path.parent() {
                cmd.current_dir(dir);
            }
            spawn_detached(&mut cmd, &path.display().to_string())
        }
        _ => Err(LaunchError::Unsupported),
    }
}

pub fn open(target: &OpenTarget) -> Result<Launched, LaunchError> {
    let arg = match target {
        OpenTarget::Url(url) => url.clone(),
        OpenTarget::Path(path) => path.display().to_string(),
    };
    // xdg-open can block until the opened program exits on some desktops,
    // so it is detached rather than awaited.
    spawn_detached(Command::new("xdg-open").arg(arg), "xdg-open")
}

/// Terminal emulators and the flags that make them run a command.
const TERMINALS: &[(&str, &[&str])] = &[
    ("xdg-terminal-exec", &[]),
    ("x-terminal-emulator", &["-e"]),
    ("kgx", &["--"]),
    ("gnome-terminal", &["--"]),
    ("ptyxis", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("mate-terminal", &["-x"]),
    ("tilix", &["-e"]),
    ("alacritty", &["-e"]),
    ("kitty", &[]),
    ("foot", &[]),
    ("wezterm", &["start", "--"]),
    ("xterm", &["-e"]),
];

fn terminal_prefix(user_choice: Option<&str>) -> Option<Vec<String>> {
    let split = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
    if let Some(choice) = user_choice.filter(|c| !c.trim().is_empty()) {
        return Some(split(choice));
    }
    if let Ok(term) = std::env::var("TERMINAL")
        && !term.is_empty()
        && find_in_path(&term).is_some()
    {
        return Some(vec![term, "-e".to_owned()]);
    }
    TERMINALS.iter().find_map(|(program, flags)| {
        find_in_path(program).map(|_| {
            let mut prefix = vec![(*program).to_owned()];
            prefix.extend(flags.iter().map(|f| (*f).to_owned()));
            prefix
        })
    })
}

pub fn run_command(
    line: &str,
    cwd: Option<&Path>,
    terminal: bool,
    terminal_override: Option<&str>,
) -> Result<Launched, LaunchError> {
    let shell = user_shell();
    let (program, args) = if terminal {
        let mut argv = terminal_prefix(terminal_override).ok_or(LaunchError::NoTerminal)?;
        // Keep the shell open afterwards so the output stays readable.
        let keep_open = format!("{line}; exec {} -l", crate::unix::sh_quote(&shell));
        argv.extend([shell.clone(), "-l".to_owned(), "-c".to_owned(), keep_open]);
        let program = argv.remove(0);
        (program, argv)
    } else {
        (
            shell.clone(),
            vec!["-l".to_owned(), "-c".to_owned(), line.to_owned()],
        )
    };
    let mut cmd = Command::new(&program);
    cmd.args(&args);
    cmd.current_dir(cwd.map(Path::to_path_buf).unwrap_or_else(home));
    spawn_detached(&mut cmd, &program)
}

fn autostart_entry() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("autostart/roshan.desktop"))
}

pub fn start_at_login() -> bool {
    autostart_entry()
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|text| DesktopEntry::parse(&text))
        .is_some_and(|entry| {
            entry.get("Hidden") != Some("true")
                && entry.get("X-GNOME-Autostart-enabled") != Some("false")
        })
}

pub fn set_start_at_login(enabled: bool, exe: &Path) -> Result<(), String> {
    let path = autostart_entry().ok_or("no config folder")?;
    if !enabled {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    let exe = exe.to_string_lossy().replace('"', "\\\"");
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Roshan\nExec=\"{exe}\" {}\nX-GNOME-Autostart-enabled=true\n",
        crate::STARTUP_FLAG
    );
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&path, entry).map_err(|e| e.to_string())
}

pub fn claim_single_instance() -> bool {
    true
}

pub fn system_language() -> Option<String> {
    language_from_env()
}

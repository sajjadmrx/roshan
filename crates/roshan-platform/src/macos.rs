//! macOS integration.
//!
//! * Discovery scans the standard application folders (two levels deep) for
//!   `.app` bundles. No Spotlight queries, no background indexing.
//! * Names come from `NSFileManager` (localized display names).
//! * Icons are rendered by `NSWorkspace`, which understands both `.icns` and
//!   asset catalogs, into a 64px bitmap.
//! * Apps and links open through `open(1)`, which reports failures; commands
//!   run in the user's login shell, either in Terminal or in the background.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use objc2::AllocAnyThread;
use objc2::rc::autoreleasepool;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSDeviceRGBColorSpace, NSGraphicsContext, NSWorkspace,
};
use objc2_foundation::{NSDictionary, NSFileManager, NSLocale, NSPoint, NSRect, NSSize, NSString};
use roshan_core::{AppTarget, LaunchError, Launched};

use crate::unix::{run_helper, sh_quote, spawn_detached, user_shell};
use crate::{DiscoveredApp, ICON_SIZE, Icon, IconFormat, OpenTarget};

const APP_DIRS: &[&str] = &[
    "/Applications",
    "/Applications/Utilities",
    "/System/Applications",
    "/System/Applications/Utilities",
];

pub fn discover_apps() -> Vec<DiscoveredApp> {
    let mut roots: Vec<PathBuf> = APP_DIRS.iter().map(PathBuf::from).collect();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }
    let mut apps = Vec::new();
    for root in roots {
        scan(&root, 2, &mut apps);
    }
    apps
}

fn scan(dir: &Path, depth: u32, out: &mut Vec<DiscoveredApp>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'));
        if hidden {
            continue;
        }
        if path.extension().is_some_and(|e| e == "app") {
            out.push(DiscoveredApp {
                name: display_name(&path),
                detail: Some(path.display().to_string()),
                target: AppTarget::MacBundle {
                    path,
                    bundle_id: None,
                },
            });
        } else if depth > 1 && path.is_dir() {
            scan(&path, depth - 1, out);
        }
    }
}

fn display_name(path: &Path) -> String {
    let name = autoreleasepool(|_| {
        let manager = NSFileManager::defaultManager();
        manager
            .displayNameAtPath(&NSString::from_str(&path.to_string_lossy()))
            .to_string()
    });
    name.strip_suffix(".app").map(str::to_owned).unwrap_or(name)
}

fn bundle_path(target: &AppTarget) -> Option<&Path> {
    match target {
        AppTarget::MacBundle { path, .. } | AppTarget::Executable { path } => Some(path),
        _ => None,
    }
}

pub fn check_app(target: &AppTarget) -> Result<(), LaunchError> {
    match bundle_path(target) {
        Some(path) if path.exists() => Ok(()),
        Some(path) => Err(LaunchError::NotFound(path.display().to_string())),
        None => Err(LaunchError::Unsupported),
    }
}

pub fn target_for_path(path: &Path) -> AppTarget {
    if path.extension().is_some_and(|e| e == "app") {
        AppTarget::MacBundle {
            path: path.to_owned(),
            bundle_id: None,
        }
    } else {
        AppTarget::Executable {
            path: path.to_owned(),
        }
    }
}

pub fn app_icon(target: &AppTarget) -> Option<Icon> {
    let path = bundle_path(target)?;
    autoreleasepool(|_| unsafe {
        let image = NSWorkspace::sharedWorkspace()
            .iconForFile(&NSString::from_str(&path.to_string_lossy()));
        let size = ICON_SIZE as isize;
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            size,
            size,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        let edge = f64::from(ICON_SIZE);
        image.drawInRect(NSRect::new(NSPoint::new(0., 0.), NSSize::new(edge, edge)));
        NSGraphicsContext::restoreGraphicsState_class();
        let png = rep
            .representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())?;
        Some(Icon {
            format: IconFormat::Png,
            bytes: png.to_vec(),
        })
    })
}

pub fn launch_app(target: &AppTarget) -> Result<Launched, LaunchError> {
    match target {
        AppTarget::MacBundle { path, .. } => {
            run_helper(Command::new("/usr/bin/open").arg("-a").arg(path), "open")
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
    run_helper(Command::new("/usr/bin/open").arg(arg), "open")
}

pub fn run_command(
    line: &str,
    cwd: Option<&Path>,
    terminal: bool,
    _terminal_override: Option<&str>,
) -> Result<Launched, LaunchError> {
    let shell = user_shell();
    if !terminal {
        let mut cmd = Command::new(&shell);
        cmd.arg("-l").arg("-c").arg(line);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        return spawn_detached(&mut cmd, &shell);
    }
    // Terminal.app runs `.command` files in a new window without needing
    // the Automation permission that AppleScript would.
    let dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Roshan");
    fs::create_dir_all(&dir).map_err(|e| crate::unix::io_error(&e, "cache"))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let script = dir.join(format!("command-{stamp}.command"));
    let cd = cwd
        .map(|d| format!("cd {} || exit 1\n", sh_quote(&d.to_string_lossy())))
        .unwrap_or_default();
    let body = format!(
        "#!/bin/sh\n{cd}rm -f \"$0\"\nexec {shell} -l -i -c {cmd}\n",
        shell = sh_quote(&shell),
        cmd = sh_quote(&format!("{line}; exec {} -l -i", sh_quote(&shell))),
    );
    fs::write(&script, body).map_err(|e| crate::unix::io_error(&e, "script"))?;
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&script, fs::Permissions::from_mode(0o700));
    }
    run_helper(
        Command::new("/usr/bin/open")
            .arg("-a")
            .arg("Terminal")
            .arg(&script),
        "Terminal",
    )
}

fn launch_agent() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join("Library/LaunchAgents/app.roshan.Roshan.plist"))
}

pub fn start_at_login() -> bool {
    launch_agent().is_some_and(|p| p.is_file())
}

pub fn set_start_at_login(enabled: bool, exe: &Path) -> Result<(), String> {
    let path = launch_agent().ok_or("no home folder")?;
    if !enabled {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>app.roshan.Roshan</string>
  <key>ProgramArguments</key>
  <array><string>{}</string><string>{}</string></array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
"#,
        escape(&exe.to_string_lossy()),
        crate::STARTUP_FLAG
    );
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&path, plist).map_err(|e| e.to_string())
}

pub fn claim_single_instance(_focus_existing: bool) -> bool {
    // LaunchServices already keeps a bundled app to one instance.
    true
}

pub fn system_language() -> Option<String> {
    autoreleasepool(|_| {
        NSLocale::preferredLanguages()
            .firstObject()
            .map(|lang| lang.to_string())
    })
    .or_else(crate::unix::language_from_env)
}

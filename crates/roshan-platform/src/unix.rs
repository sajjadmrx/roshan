//! Helpers shared by the macOS and Linux integrations.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use roshan_core::{LaunchError, Launched};

/// The user's login shell.
pub fn user_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/sh".to_owned())
}

/// Starts `cmd` detached from Roshan: own process group, no inherited
/// terminal, and a background thread that reaps it so it never lingers as a
/// zombie.
pub fn spawn_detached(cmd: &mut Command, what: &str) -> Result<Launched, LaunchError> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let child = cmd.spawn().map_err(|e| io_error(&e, what))?;
    let pid = child.id();
    reap(child);
    Ok(Launched { pid: Some(pid) })
}

pub fn reap(mut child: Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

pub fn io_error(e: &std::io::Error, what: &str) -> LaunchError {
    match e.kind() {
        std::io::ErrorKind::NotFound => LaunchError::NotFound(what.to_owned()),
        std::io::ErrorKind::PermissionDenied => LaunchError::AccessDenied,
        _ => LaunchError::Os {
            code: i64::from(e.raw_os_error().unwrap_or(0)),
            message: e.to_string(),
        },
    }
}

/// Runs a short-lived helper (`open`, `xdg-open`) and turns a non-zero exit
/// into an error carrying its message.
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub fn run_helper(cmd: &mut Command, what: &str) -> Result<Launched, LaunchError> {
    let output = cmd
        .stdin(Stdio::null())
        .output()
        .map_err(|e| io_error(&e, what))?;
    if output.status.success() {
        Ok(Launched { pid: None })
    } else {
        Err(LaunchError::Os {
            code: i64::from(output.status.code().unwrap_or(-1)),
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

/// Quotes `s` for a POSIX shell.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn find_in_path(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        let path = PathBuf::from(program);
        return path.is_file().then_some(path);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())
    })
}

pub fn language_from_env() -> Option<String> {
    ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(|value| value.split(':').next().unwrap_or_default().to_owned())
        .find(|value| !value.is_empty() && value != "C" && value != "POSIX")
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn file_name_lossy(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(sh_quote("plain"), "'plain'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }
}

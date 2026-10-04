//! Updates from GitHub Releases.
//!
//! The only network access Roshan makes. It sends a plain HTTPS request for
//! the latest published release (drafts and pre-releases are excluded by
//! GitHub's `/releases/latest`) and, on Windows, downloads the release's
//! installer, verifies it against `SHA256SUMS.txt`, and runs it silently when
//! the user chooses to restart. Nothing about the user is sent.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use roshan_core::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const RELEASES_API: &str = "https://api.github.com/repos/sajjadmrx/roshan/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/sajjadmrx/roshan/releases/latest";

/// Command-line flag the installer passes when it reopens Roshan after a
/// silent update.
pub const UPDATED_FLAG: &str = "--updated";

const CHECKSUMS: &str = "SHA256SUMS.txt";
/// Refuse absurd downloads (a Roshan build is ~25 MB).
const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;

/// Windows releases ship `Roshan-Setup-<version>.exe` (see
/// `packaging/windows/roshan.iss`). Other systems update by hand.
const INSTALLER_PREFIX: &str = "Roshan-Setup-";
const SELF_UPDATES: bool = cfg!(all(windows, target_arch = "x86_64"));

fn is_installer(name: &str) -> bool {
    name.starts_with(INSTALLER_PREFIX) && name.ends_with(".exe")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// The release page, for platforms that update by hand.
    pub page: String,
    /// The file to install on this platform, if any.
    pub installer: Option<Asset>,
    pub checksums: Option<Asset>,
}

impl Release {
    /// Whether this release can be downloaded and installed automatically.
    pub fn installable(&self) -> bool {
        self.installer.is_some() && self.checksums.is_some()
    }
}

pub type Result<T> = std::result::Result<T, String>;

/// `ROSHAN_UPDATE_URL` points the check at another server (for testing).
fn api_url() -> String {
    std::env::var("ROSHAN_UPDATE_URL").unwrap_or_else(|_| RELEASES_API.to_owned())
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        // Respect HTTPS_PROXY / ALL_PROXY, common where GitHub needs a proxy.
        .proxy(ureq::Proxy::try_from_env())
        .user_agent(format!("Roshan/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn get_text(url: &str, timeout: Duration) -> Result<String> {
    let mut response = agent(timeout)
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Asks GitHub for the latest published release.
pub fn latest_release() -> Result<Release> {
    parse_release(&get_text(&api_url(), Duration::from_secs(20))?)
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

pub fn parse_release(json: &str) -> Result<Release> {
    let release: GhRelease = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if release.draft || release.prerelease {
        return Err("the latest release is not published yet".into());
    }
    let version = Version::parse(&release.tag_name)
        .ok_or_else(|| format!("unexpected release tag {}", release.tag_name))?;
    let find = |pred: &dyn Fn(&str) -> bool| {
        release
            .assets
            .iter()
            .find(|a| pred(&a.name))
            .map(|a| Asset {
                name: a.name.clone(),
                url: a.browser_download_url.clone(),
                size: a.size,
            })
    };
    let installer = if SELF_UPDATES {
        find(&is_installer)
    } else {
        None
    };
    let checksums = find(&|name| name == CHECKSUMS);
    Ok(Release {
        version,
        page: release.html_url,
        installer,
        checksums,
    })
}

/// Finds `file`'s hash in `sha256sum` output (`<hex>  <name>` per line).
pub fn expected_checksum(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name == file && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// Where downloads wait until the user restarts.
pub fn updates_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Roshan")
        .join("updates")
}

/// Downloads the release's installer and verifies its checksum. Returns the
/// verified file, ready for [`install_and_restart`].
pub fn download(release: &Release) -> Result<PathBuf> {
    let (Some(installer), Some(checksums)) = (&release.installer, &release.checksums) else {
        return Err("this release has nothing to install on this system".into());
    };
    let sums = get_text(&checksums.url, Duration::from_secs(30))?;
    let expected = expected_checksum(&sums, &installer.name)
        .ok_or_else(|| format!("{} is not listed in {CHECKSUMS}", installer.name))?;

    let dir = updates_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(&installer.name);
    if target.is_file() && sha256_file(&target).ok().as_deref() == Some(expected.as_str()) {
        return Ok(target); // Already downloaded earlier.
    }

    let partial = dir.join(format!("{}.part", installer.name));
    let mut response = agent(Duration::from_secs(15 * 60))
        .get(&installer.url)
        .call()
        .map_err(|e| e.to_string())?;
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD)
        .reader();
    let mut file = fs::File::create(&partial).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    drop(file);

    let actual = sha256_file(&partial).map_err(|e| e.to_string())?;
    if actual != expected {
        let _ = fs::remove_file(&partial);
        return Err("the download did not match its checksum".into());
    }
    fs::rename(&partial, &target).map_err(|e| e.to_string())?;
    Ok(target)
}

/// Runs the downloaded installer silently. It closes this copy of Roshan,
/// replaces the files and reopens Roshan with [`UPDATED_FLAG`]. The caller
/// quits right after this returns `Ok`.
pub fn install_and_restart(installer: &Path) -> Result<()> {
    if !SELF_UPDATES {
        return Err("Roshan cannot update itself on this system yet".into());
    }
    std::process::Command::new(installer)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
            // Tells the installer to reopen Roshan when it is done.
            "/RELAUNCH=1",
        ])
        .spawn()
        .map_err(|e| format!("cannot start the installer: {e}"))?;
    Ok(())
}

/// Version of a downloaded installer, from its file name.
fn installer_version(name: &str) -> Option<Version> {
    name.strip_prefix(INSTALLER_PREFIX)?
        .strip_suffix(".exe")
        .and_then(Version::parse)
}

/// Removes leftovers of a finished update. Call once at startup.
pub fn clean_up() {
    let current = Version::parse(env!("CARGO_PKG_VERSION"));
    if let Ok(entries) = fs::read_dir(updates_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Keep an installer that is newer than this build; drop the rest.
            let newer = installer_version(&name)
                .zip(current.clone())
                .is_some_and(|(found, current)| found > current);
            if !newer {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "tag_name": "v0.2.0",
        "html_url": "https://github.com/sajjadmrx/roshan/releases/tag/v0.2.0",
        "draft": false,
        "prerelease": false,
        "assets": [
            {"name": "Roshan-Setup-0.2.0.exe", "browser_download_url": "https://example.com/w.exe", "size": 100},
            {"name": "roshan-0.2.0-linux-x64.tar.gz", "browser_download_url": "https://example.com/l.tgz", "size": 90},
            {"name": "SHA256SUMS.txt", "browser_download_url": "https://example.com/sums", "size": 10}
        ]
    }"#;

    #[test]
    fn parses_a_github_release() {
        let release = parse_release(SAMPLE).unwrap();
        assert_eq!(release.version, Version::parse("0.2.0").unwrap());
        assert!(release.page.ends_with("/v0.2.0"));
        assert_eq!(release.checksums.unwrap().name, "SHA256SUMS.txt");
        if cfg!(all(windows, target_arch = "x86_64")) {
            assert_eq!(release.installer.unwrap().url, "https://example.com/w.exe");
        } else {
            assert!(release.installer.is_none());
        }
    }

    #[test]
    fn ignores_drafts_prereleases_and_odd_tags() {
        let draft = SAMPLE.replace(r#""draft": false"#, r#""draft": true"#);
        assert!(parse_release(&draft).is_err());
        let pre = SAMPLE.replace(r#""prerelease": false"#, r#""prerelease": true"#);
        assert!(parse_release(&pre).is_err());
        let odd = SAMPLE.replace("v0.2.0\"", "nightly\"");
        assert!(parse_release(&odd).is_err());
    }

    #[test]
    fn reads_installer_versions() {
        assert_eq!(
            installer_version("Roshan-Setup-1.2.3.exe"),
            Version::parse("1.2.3")
        );
        assert_eq!(installer_version("Roshan-Setup-1.2.3.exe.part"), None);
        assert_eq!(installer_version("roshan-1.2.3-windows-x64.exe"), None);
    }

    #[test]
    fn reads_sha256sum_output() {
        let hash = "a".repeat(64);
        let sums = format!(
            "{hash}  Roshan-Setup-0.2.0.exe\n{}  other.tar.gz\n",
            "b".repeat(64)
        );
        assert_eq!(
            expected_checksum(&sums, "Roshan-Setup-0.2.0.exe"),
            Some(hash)
        );
        assert_eq!(expected_checksum(&sums, "missing.exe"), None);
        assert_eq!(expected_checksum("nothex  x.exe", "x.exe"), None);
    }

    #[test]
    fn hashes_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

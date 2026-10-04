//! Validation for "Open" items (URL, file or folder).

use std::path::PathBuf;

use roshan_core::LaunchError;

/// URL schemes Roshan will hand to the OS. Other schemes can trigger
/// arbitrary protocol handlers (`ms-msdt:` and friends), so they are refused.
pub const ALLOWED_SCHEMES: &[&str] = &["http", "https", "mailto", "ftp", "file"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenTarget {
    Url(String),
    Path(PathBuf),
}

/// Decides whether `target` is a URL or a local path and validates it.
pub fn classify(target: &str) -> Result<OpenTarget, LaunchError> {
    let target = target.trim();
    if target.is_empty() {
        return Err(LaunchError::NotFound(String::new()));
    }
    if let Some(scheme) = url_scheme(target) {
        let scheme = scheme.to_ascii_lowercase();
        return if ALLOWED_SCHEMES.contains(&scheme.as_str()) {
            Ok(OpenTarget::Url(target.to_owned()))
        } else {
            Err(LaunchError::Blocked(format!("{scheme}:")))
        };
    }
    if target.starts_with("www.") {
        return Ok(OpenTarget::Url(format!("https://{target}")));
    }
    let path = expand_home(target);
    if path.exists() {
        Ok(OpenTarget::Path(path))
    } else if looks_like_domain(target) {
        Ok(OpenTarget::Url(format!("https://{target}")))
    } else {
        Err(LaunchError::NotFound(target.to_owned()))
    }
}

/// "github.com" or "example.org/docs": a host with a dot and a letter TLD.
fn looks_like_domain(s: &str) -> bool {
    // Missing files like "notes.txt" must not turn into web links.
    const FILE_EXTENSIONS: &[&str] = &[
        "txt", "pdf", "exe", "md", "doc", "docx", "xls", "xlsx", "png", "jpg", "jpeg", "gif",
        "lnk", "bat", "cmd", "ps1", "sh", "json", "toml", "log", "ini", "xml", "html", "htm",
    ];
    if s.contains(char::is_whitespace) || s.contains('\\') || s.starts_with(['.', '/', '~']) {
        return false;
    }
    let host = s.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default();
    match host.rsplit_once('.') {
        Some((name, tld)) => {
            !name.is_empty()
                && tld.len() >= 2
                && tld.chars().all(|c| c.is_ascii_alphabetic())
                && !FILE_EXTENSIONS.contains(&tld.to_ascii_lowercase().as_str())
                && host
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-'))
        }
        None => false,
    }
}

/// Returns the scheme of `s` if it looks like `scheme:...`. A single letter
/// followed by `:` is a Windows drive, not a scheme.
fn url_scheme(s: &str) -> Option<&str> {
    let (scheme, _) = s.split_once(':')?;
    let valid = scheme.len() > 1
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then_some(scheme)
}

fn expand_home(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\"))
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_web_urls() {
        assert_eq!(
            classify("https://example.com").unwrap(),
            OpenTarget::Url("https://example.com".into())
        );
        assert_eq!(
            classify("www.example.com").unwrap(),
            OpenTarget::Url("https://www.example.com".into())
        );
    }

    #[test]
    fn bare_domains_become_https_links() {
        assert_eq!(
            classify("github.com").unwrap(),
            OpenTarget::Url("https://github.com".into())
        );
        assert_eq!(
            classify("example.org/docs?x=1").unwrap(),
            OpenTarget::Url("https://example.org/docs?x=1".into())
        );
        assert!(classify("missing-notes.txt").is_err());
        assert!(classify("no spaces.com").is_err());
    }

    #[test]
    fn blocks_unknown_schemes() {
        assert_eq!(
            classify("ms-msdt:/id PCWDiagnostic").unwrap_err(),
            LaunchError::Blocked("ms-msdt:".into())
        );
    }

    #[test]
    fn drive_letters_are_paths_not_schemes() {
        assert_eq!(url_scheme("C:\\Users"), None);
        assert!(matches!(
            classify("Z:\\definitely\\missing\\path"),
            Err(LaunchError::NotFound(_))
        ));
    }

    #[test]
    fn existing_paths_are_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().to_string_lossy().into_owned();
        assert_eq!(
            classify(&target).unwrap(),
            OpenTarget::Path(dir.path().to_owned())
        );
    }
}

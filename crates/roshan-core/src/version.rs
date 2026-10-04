//! Release versions (`1.2.3`, `1.2.3-beta.1`) and their ordering, for the
//! update check.

use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Pre-release label, e.g. `beta.1`. A pre-release sorts before the
    /// release with the same numbers.
    pub pre: Option<String>,
}

impl Version {
    /// Parses `1.2.3`, `v1.2.3` or `1.2.3-beta.1`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches(['v', 'V']);
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) if !pre.is_empty() => (core, Some(pre.to_owned())),
            Some(_) => return None,
            None => (text, None),
        };
        let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
        let version = Self {
            major: parts.next()??,
            minor: parts.next()??,
            patch: parts.next()??,
            pre,
        };
        parts.next().is_none().then_some(version)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn parses_tags_and_plain_versions() {
        assert_eq!(v("v1.2.3"), v("1.2.3"));
        assert_eq!(v("0.10.0").minor, 10);
        assert_eq!(v("2.0.0-beta.1").pre.as_deref(), Some("beta.1"));
        assert_eq!(v("1.2.3-rc.2").to_string(), "1.2.3-rc.2");
    }

    #[test]
    fn rejects_malformed_versions() {
        for bad in ["", "1.2", "1.2.3.4", "1.x.3", "1.2.3-", "latest"] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn orders_numerically_and_prereleases_first() {
        assert!(v("0.10.0") > v("0.9.9"));
        assert!(v("1.0.0") > v("1.0.0-beta.2"));
        assert!(v("1.0.0-beta.2") > v("1.0.0-beta.1"));
        assert!(v("1.0.1") > v("1.0.0"));
        assert_eq!(v("1.0.0").cmp(&v("v1.0.0")), Ordering::Equal);
    }
}

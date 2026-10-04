use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Upper bound for a single "wait after" delay. Long enough for slow VPN
/// clients, short enough that a typo cannot stall a session for hours.
pub const MAX_WAIT_SECS: u32 = 600;

/// A named, ordered list of things to launch together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub name: String,
    /// Optional single emoji or short glyph shown next to the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge: Option<String>,
    #[serde(default, rename = "item")]
    pub items: Vec<LaunchItem>,
}

impl Session {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            badge: None,
            items: Vec::new(),
        }
    }

    /// Moves the item at `from` so that it ends up at index `to`.
    /// Out-of-range indices are ignored.
    pub fn move_item(&mut self, from: usize, to: usize) {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
    }
}

/// One step of a session. Its position in [`Session::items`] is its launch order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaunchItem {
    /// Display name. For applications this is the name the OS reported when
    /// the item was added, so a later uninstall still shows something useful.
    pub name: String,
    #[serde(flatten)]
    pub kind: ItemKind,
    /// Seconds to wait after this item was launched before launching the next.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub wait_after_secs: u32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

impl LaunchItem {
    pub fn new(name: impl Into<String>, kind: ItemKind) -> Self {
        Self {
            name: name.into(),
            kind,
            wait_after_secs: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemKind {
    /// An application discovered on this machine (or picked by hand).
    App { target: AppTarget },
    /// A command line executed by the user's shell.
    Command {
        line: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<PathBuf>,
        /// Open a visible terminal window (default) or run without a window.
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        terminal: bool,
    },
    /// A URL, file or folder opened with the system's default handler.
    Open { target: String },
}

fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

/// An OS-native reference to an application. These are deliberately
/// platform-specific: a session file is a per-machine configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppTarget {
    /// Windows: an entry of the shell's AppsFolder ("All apps" in Start),
    /// identified by its parsing name (an AppUserModelID or a known-folder path).
    WindowsApp { id: String },
    /// macOS: an application bundle.
    MacBundle {
        path: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bundle_id: Option<String>,
    },
    /// Linux and other freedesktop systems: a `.desktop` entry.
    DesktopEntry { id: String, path: PathBuf },
    /// Any executable chosen by hand with "Browse…".
    Executable { path: PathBuf },
}

impl AppTarget {
    /// A stable string used as a cache key for icons.
    pub fn cache_key(&self) -> String {
        match self {
            AppTarget::WindowsApp { id } => format!("win:{id}"),
            AppTarget::MacBundle { path, .. } => format!("mac:{}", path.display()),
            AppTarget::DesktopEntry { path, .. } => format!("xdg:{}", path.display()),
            AppTarget::Executable { path } => format!("exe:{}", path.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with(n: usize) -> Session {
        let mut s = Session::new("s");
        for i in 0..n {
            s.items.push(LaunchItem::new(
                format!("item{i}"),
                ItemKind::Open {
                    target: format!("https://example.com/{i}"),
                },
            ));
        }
        s
    }

    fn names(s: &Session) -> Vec<&str> {
        s.items.iter().map(|i| i.name.as_str()).collect()
    }

    #[test]
    fn move_item_down_and_up() {
        let mut s = session_with(4);
        s.move_item(0, 2);
        assert_eq!(names(&s), ["item1", "item2", "item0", "item3"]);
        s.move_item(3, 0);
        assert_eq!(names(&s), ["item3", "item1", "item2", "item0"]);
    }

    #[test]
    fn move_item_ignores_out_of_range() {
        let mut s = session_with(2);
        s.move_item(0, 5);
        s.move_item(7, 0);
        assert_eq!(names(&s), ["item0", "item1"]);
    }
}

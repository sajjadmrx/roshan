use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

/// Roshan's own icons (a small subset of Lucide, plus the logo), falling back
/// to the component library's built-in icons.
#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/*.svg"]
struct Embedded;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match Embedded::get(path) {
            Some(file) => Ok(Some(file.data)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names: Vec<SharedString> = Embedded::iter()
            .filter(|name| name.starts_with(path))
            .map(|name| name.to_string().into())
            .collect();
        names.extend(gpui_kit::assets::Assets.list(path)?);
        names.sort();
        names.dedup();
        Ok(names)
    }
}

/// Session badges a user can choose from (Lucide icon names).
pub const BADGES: &[&str] = &[
    "sunrise",
    "sun",
    "sunset",
    "moon",
    "briefcase",
    "code",
    "terminal",
    "gamepad-2",
    "clapperboard",
    "headphones",
    "music",
    "book-open",
    "graduation-cap",
    "flask-conical",
    "palette",
    "coffee",
    "rocket",
    "zap",
    "heart",
    "globe",
];

/// Vazirmatn (SIL OFL 1.1, see assets/fonts/OFL.txt), the typeface for the
/// Persian interface. Bundled so Persian looks the same on every machine.
const FONTS: [&[u8]; 3] = [
    include_bytes!("../assets/fonts/Vazirmatn-Regular.ttf"),
    include_bytes!("../assets/fonts/Vazirmatn-Medium.ttf"),
    include_bytes!("../assets/fonts/Vazirmatn-Bold.ttf"),
];

pub const PERSIAN_FONT: &str = "Vazirmatn";

pub fn load_fonts(cx: &gpui_kit::App) {
    let fonts = FONTS.iter().map(|f| Cow::Borrowed(*f)).collect();
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        eprintln!("roshan: could not load the Persian font: {err}");
    }
}

pub fn icon_path(name: &str) -> SharedString {
    format!("icons/{name}.svg").into()
}

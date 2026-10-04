//! A small on-disk cache of extracted icons, so the picker opens instantly
//! after the first time. Entries expire after a while so updated application
//! icons are eventually picked up.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::{ICON_SIZE, Icon, IconFormat};

const MAX_AGE: Duration = Duration::from_secs(14 * 24 * 60 * 60);
/// Bump when the extraction changes, to invalidate old entries.
const VERSION: u32 = 1;

fn dir() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("Roshan").join("icons"))
}

/// FNV-1a: stable across Rust versions, unlike `DefaultHasher`.
fn hash(key: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in format!("{VERSION}:{ICON_SIZE}:{key}").bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn path(key: &str, format: IconFormat) -> Option<PathBuf> {
    let ext = match format {
        IconFormat::Png => "png",
        IconFormat::Svg => "svg",
    };
    Some(dir()?.join(format!("{:016x}.{ext}", hash(key))))
}

pub fn get(key: &str) -> Option<Icon> {
    for format in [IconFormat::Png, IconFormat::Svg] {
        let path = path(key, format)?;
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        let fresh = meta
            .modified()
            .ok()
            .and_then(|m| SystemTime::now().duration_since(m).ok())
            .is_some_and(|age| age < MAX_AGE);
        if !fresh {
            let _ = fs::remove_file(&path);
            return None;
        }
        return fs::read(&path).ok().map(|bytes| Icon { format, bytes });
    }
    None
}

pub fn put(key: &str, icon: &Icon) {
    let Some(path) = path(key, icon.format) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    // Write-then-rename so a concurrent reader never sees a partial file.
    let tmp = path.with_extension("tmp");
    if fs::write(&tmp, &icon.bytes).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

/// Encodes straight (non-premultiplied) RGBA pixels as PNG.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn encode_png(width: u32, height: u32, rgba: Vec<u8>) -> Option<Icon> {
    let image = image::RgbaImage::from_raw(width, height, rgba)?;
    let image = if width > ICON_SIZE || height > ICON_SIZE {
        image::imageops::resize(
            &image,
            ICON_SIZE,
            ICON_SIZE,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        image
    };
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .ok()?;
    Some(Icon {
        format: IconFormat::Png,
        bytes,
    })
}

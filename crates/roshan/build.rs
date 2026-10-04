//! Prepares Roshan's logo: the master render (`assets/logo/roshan.png`, made
//! in Blender from `assets/logo/roshan-logo.blend`) is downscaled for the UI
//! and, on Windows, embedded with version information into the executable.

use std::path::Path;

const MASTER: &str = "assets/logo/roshan.png";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={MASTER}");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let out = Path::new(&out_dir);

    let master = image::open(MASTER).expect("read the logo").to_rgba8();
    // Pre-scaled copies, so the UI never samples the 1024px master down.
    for size in [64u32, 192] {
        resize(&master, size)
            .save(out.join(format!("roshan-{size}.png")))
            .expect("write a logo size");
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        write_ico(&master, &out.join("roshan.ico"));
        write_installer_art(&master, out);
        let version = std::env::var("CARGO_PKG_VERSION").unwrap();
        let numeric = version.replace('.', ",") + ",0";
        let rc = format!(
            r#"#pragma code_page(65001)
1 ICON "roshan.ico"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Roshan contributors"
      VALUE "FileDescription", "Roshan"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "roshan"
      VALUE "OriginalFilename", "roshan.exe"
      VALUE "ProductName", "Roshan"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
        );
        let rc_path = out.join("roshan.rc");
        std::fs::write(&rc_path, rc).expect("write roshan.rc");
        embed_resource::compile(&rc_path, embed_resource::NONE)
            .manifest_optional()
            .expect("embed Windows resources");
    }
}

fn resize(master: &image::RgbaImage, size: u32) -> image::RgbaImage {
    image::imageops::resize(master, size, size, image::imageops::FilterType::Lanczos3)
}

/// Pictures for the Inno Setup wizard (packaging/windows/roshan.iss), at 2x
/// for high-DPI screens: the logo for the page header, and a charcoal panel
/// with the logo for the first and last pages.
fn write_installer_art(master: &image::RgbaImage, out: &Path) {
    let charcoal = image::Rgba([0x14, 0x13, 0x12, 0xff]);

    let mut small = image::RgbaImage::from_pixel(110, 110, image::Rgba([0xff, 0xff, 0xff, 0xff]));
    image::imageops::overlay(&mut small, &resize(master, 96), 7, 7);
    image::DynamicImage::ImageRgba8(small)
        .to_rgb8()
        .save(out.join("wizard-small.bmp"))
        .expect("write wizard-small.bmp");

    let (w, h) = (328u32, 628u32);
    let mut large = image::RgbaImage::from_pixel(w, h, charcoal);
    let logo = resize(master, 176);
    image::imageops::overlay(
        &mut large,
        &logo,
        i64::from((w - 176) / 2),
        i64::from(h / 2 - 140),
    );
    image::DynamicImage::ImageRgba8(large)
        .to_rgb8()
        .save(out.join("wizard-large.bmp"))
        .expect("write wizard-large.bmp");
}

fn write_ico(master: &image::RgbaImage, path: &Path) {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    let frames: Vec<IcoFrame> = [16u32, 20, 24, 32, 40, 48, 64, 128, 256]
        .iter()
        .map(|&size| {
            let img = resize(master, size);
            IcoFrame::as_png(img.as_raw(), size, size, image::ExtendedColorType::Rgba8)
                .expect("encode icon frame")
        })
        .collect();
    let file = std::fs::File::create(path).expect("create roshan.ico");
    IcoEncoder::new(file)
        .encode_images(&frames)
        .expect("write roshan.ico");
}

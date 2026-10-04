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

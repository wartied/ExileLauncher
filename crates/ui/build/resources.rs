//! Windows resources for Exile's executables: the icon (from `src/icon.rs`),
//! the name and version shown in Explorer and Task Manager, and optionally a
//! manifest. The launcher's and the installer's build scripts include this
//! file and `src/icon.rs` with `#[path]`; it needs the `winresource` and
//! `png` build dependencies.

use std::path::Path;

pub struct Details<'a> {
    /// Shown as the file's description, e.g. in Task Manager.
    pub description: &'a str,
    /// "major.minor.patch".
    pub version: &'a str,
    pub manifest: Option<&'a str>,
}

/// Compiles the resources into the executable. Without the Windows SDK's
/// resource compiler this fails: a warning if `required` is false (the
/// executable just has no icon), a build error otherwise.
pub fn embed(out_dir: &Path, details: Details, required: bool) {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = out_dir.join("exile.ico");
    std::fs::write(&ico, crate::icon::ico(&[16, 20, 24, 32, 40, 48, 64, 256], encode_png)).expect("writing exile.ico");

    let mut res = winresource::WindowsResource::new();
    res.set_icon(&ico.to_string_lossy())
        .set("FileDescription", details.description)
        .set("ProductName", "Exile")
        .set("CompanyName", "Exile")
        .set("FileVersion", details.version)
        .set("ProductVersion", details.version)
        .set_version_info(winresource::VersionInfo::FILEVERSION, packed(details.version))
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed(details.version));
    if let Some(manifest) = details.manifest {
        res.set_manifest(manifest);
    }
    if let Err(e) = res.compile() {
        if required {
            panic!("compiling Windows resources (needs the Windows SDK): {e}");
        }
        println!("cargo::warning=no icon in the executable: compiling Windows resources failed ({e})");
    }
}

/// "0.2.2" -> 0x0000_0002_0002_0000, the layout of a VERSIONINFO number.
fn packed(version: &str) -> u64 {
    let mut parts = version.split(['.', '-']).map(|p| p.parse::<u64>().unwrap_or(0));
    (0..4).fold(0, |acc, _| (acc << 16) | (parts.next().unwrap_or(0) & 0xffff))
}

fn encode_png(rgba: &[u8], size: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("encoding the icon");
    writer.write_image_data(rgba).expect("encoding the icon");
    writer.finish().expect("encoding the icon");
    out
}

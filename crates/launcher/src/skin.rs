//! The account avatar: the face of the player's skin, with the hat layer.

use std::fs;

use anyhow::{Context as _, Result};

use crate::{net, paths};

/// 8x8 RGBA pixels.
pub type Face = Vec<u8>;

fn cache_path() -> std::path::PathBuf {
    paths::root().join("skin.png")
}

/// Fetches the skin (falling back to the cached copy) and extracts the face.
pub fn load_face(skin_url: Option<&str>) -> Result<Face> {
    let png = match skin_url.map(net::get_bytes) {
        Some(Ok(bytes)) => {
            let _ = fs::write(cache_path(), &bytes);
            bytes
        }
        _ => fs::read(cache_path()).context("no skin available")?,
    };
    face_from_png(&png)
}

pub fn cached_face() -> Option<Face> {
    face_from_png(&fs::read(cache_path()).ok()?).ok()
}

pub fn clear_cache() {
    let _ = fs::remove_file(cache_path());
}

fn face_from_png(png: &[u8]) -> Result<Face> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)?.to_rgba8();
    if img.width() < 64 || img.height() < 32 {
        anyhow::bail!("skin image is too small");
    }
    // Skins may be HD (multiples of 64 px wide); sample at the base scale.
    let scale = img.width() / 64;
    let mut out = Vec::with_capacity(8 * 8 * 4);
    for y in 0..8 {
        for x in 0..8 {
            let base = img.get_pixel((8 + x) * scale, (8 + y) * scale).0;
            let hat = img.get_pixel((40 + x) * scale, (8 + y) * scale).0;
            out.extend_from_slice(&blend(base, hat));
        }
    }
    Ok(out)
}

fn blend(base: [u8; 4], top: [u8; 4]) -> [u8; 4] {
    let a = top[3] as u32;
    let mix = |b: u8, t: u8| ((t as u32 * a + b as u32 * (255 - a)) / 255) as u8;
    [mix(base[0], top[0]), mix(base[1], top[1]), mix(base[2], top[2]), 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_face_with_hat() {
        let mut img = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 0]));
        img.put_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(9, 8, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(41, 8, image::Rgba([0, 0, 255, 255]));
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let face = face_from_png(&png).unwrap();
        assert_eq!(&face[0..4], &[255, 0, 0, 255]);
        assert_eq!(&face[4..8], &[0, 0, 255, 255]);
    }
}

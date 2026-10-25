//! Player skins: the face for avatars, and the whole texture for the 3D
//! preview in Settings. One cached copy per account, so every account in the
//! switcher has its face even offline.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};

use crate::{net, paths};

/// 8x8 RGBA pixels.
pub type Face = Vec<u8>;

pub struct Skin {
    /// The face with the hat layer on top, 8x8 RGBA.
    pub face: Face,
    /// The skin at base resolution: 64x64 RGBA (64x32 skins are padded).
    pub texture: Vec<u8>,
    /// A pre-1.8 64x32 skin: no second layer except the hat, and the left
    /// arm and leg mirror the right ones.
    pub legacy: bool,
}

fn cache_path(uuid: &str) -> PathBuf {
    // UUIDs come from Mojang, but never let one choose a path.
    let safe: String = uuid.chars().filter(|c| c.is_ascii_hexdigit() || *c == '-').collect();
    paths::root().join("skins").join(format!("{safe}.png"))
}

/// Fetches the account's skin (falling back to the cached copy).
pub fn load(uuid: &str, skin_url: Option<&str>) -> Result<Skin> {
    let path = cache_path(uuid);
    let png = match skin_url.map(net::get_bytes) {
        Some(Ok(bytes)) => {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&path, &bytes);
            bytes
        }
        _ => fs::read(&path).context("no skin available")?,
    };
    decode(&png)
}

pub fn cached(uuid: &str) -> Option<Skin> {
    decode(&fs::read(cache_path(uuid)).ok()?).ok()
}

pub fn forget(uuid: &str) {
    let _ = fs::remove_file(cache_path(uuid));
}

fn decode(png: &[u8]) -> Result<Skin> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)?.to_rgba8();
    let (w, h) = img.dimensions();
    if w < 64 || w % 64 != 0 || (h != w && h != w / 2) {
        bail!("not a skin image ({w}x{h})");
    }
    // HD skins are multiples of 64 px wide; sample them at the base scale.
    let scale = w / 64;
    let legacy = h == w / 2;
    let rows = if legacy { 32 } else { 64 };
    let mut texture = vec![0u8; 64 * 64 * 4];
    for y in 0..rows {
        for x in 0..64 {
            let px = img.get_pixel(x * scale, y * scale).0;
            texture[((y * 64 + x) * 4) as usize..][..4].copy_from_slice(&px);
        }
    }
    let mut face = Vec::with_capacity(8 * 8 * 4);
    for y in 0..8u32 {
        for x in 0..8u32 {
            let at = |u: u32, v: u32| {
                let i = ((v * 64 + u) * 4) as usize;
                [texture[i], texture[i + 1], texture[i + 2], texture[i + 3]]
            };
            face.extend_from_slice(&blend(at(8 + x, 8 + y), at(40 + x, 8 + y)));
        }
    }
    Ok(Skin { face, texture, legacy })
}

fn blend(base: [u8; 4], top: [u8; 4]) -> [u8; 4] {
    let a = top[3] as u32;
    let mix = |b: u8, t: u8| ((t as u32 * a + b as u32 * (255 - a)) / 255) as u8;
    [mix(base[0], top[0]), mix(base[1], top[1]), mix(base[2], top[2]), 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(img: &image::RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    #[test]
    fn extracts_face_with_hat() {
        let mut img = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 0]));
        img.put_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(9, 8, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(41, 8, image::Rgba([0, 0, 255, 255]));
        let skin = decode(&png(&img)).unwrap();
        assert_eq!(&skin.face[0..4], &[255, 0, 0, 255]);
        assert_eq!(&skin.face[4..8], &[0, 0, 255, 255]);
        assert!(!skin.legacy);
        assert_eq!(skin.texture.len(), 64 * 64 * 4);
    }

    #[test]
    fn legacy_and_hd_skins() {
        let legacy = image::RgbaImage::from_pixel(64, 32, image::Rgba([1, 2, 3, 255]));
        let s = decode(&png(&legacy)).unwrap();
        assert!(s.legacy);
        // The missing bottom half stays transparent.
        assert_eq!(s.texture[(40 * 64 + 10) * 4 + 3], 0);

        let mut hd = image::RgbaImage::from_pixel(128, 128, image::Rgba([0, 0, 0, 0]));
        hd.put_pixel(16, 16, image::Rgba([9, 9, 9, 255]));
        assert_eq!(&decode(&png(&hd)).unwrap().face[0..4], &[9, 9, 9, 255]);

        assert!(decode(&png(&image::RgbaImage::new(64, 48))).is_err());
    }

    #[test]
    fn cache_path_ignores_odd_characters() {
        let p = cache_path("../../evil");
        assert!(p.ends_with("skins/e.png") || p.ends_with("skins\\e.png"), "{p:?}");
    }
}

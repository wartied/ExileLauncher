//! Player skins and capes: the face for avatars, and the whole texture for
//! the 3D preview in Settings. One cached copy per account, so every account
//! in the switcher has its face even offline.
//!
//! The 3D texture is one 64x96 image: the skin in the top 64 rows and the
//! cape (64x32) below it, so the whole model draws as one mesh.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};

use crate::{net, paths};

/// 8x8 RGBA pixels.
pub type Face = Vec<u8>;

/// Size of the combined skin and cape texture.
pub const ATLAS: [usize; 2] = [64, 96];

pub struct Skin {
    /// The face with the hat layer on top, 8x8 RGBA.
    pub face: Face,
    /// Skin (rows 0..64; 64x32 skins are padded) and cape (rows 64..96), RGBA.
    pub texture: Vec<u8>,
    /// A pre-1.8 64x32 skin: no second layer except the hat, and the left
    /// arm and leg mirror the right ones.
    pub legacy: bool,
    /// The image is laid out for slim (3 px) arms, whatever the profile says.
    pub slim: bool,
    pub cape: bool,
}

fn cache_path(uuid: &str, what: &str) -> PathBuf {
    // UUIDs come from Mojang, but never let one choose a path.
    let safe: String = uuid.chars().filter(|c| c.is_ascii_hexdigit() || *c == '-').collect();
    paths::root().join("skins").join(format!("{safe}{what}.png"))
}

/// Downloads `url` into the cache, or falls back to the cached copy.
fn fetch(url: Option<&str>, path: &PathBuf) -> Option<Vec<u8>> {
    match url.map(net::get_bytes) {
        Some(Ok(bytes)) => {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(path, &bytes);
            Some(bytes)
        }
        // No cape any more: forget the old one.
        None => {
            let _ = fs::remove_file(path);
            None
        }
        Some(Err(_)) => fs::read(path).ok(),
    }
}

/// Fetches the account's skin and cape (falling back to cached copies).
pub fn load(uuid: &str, skin_url: Option<&str>, cape_url: Option<&str>) -> Result<Skin> {
    let skin_path = cache_path(uuid, "");
    let png = match skin_url.map(net::get_bytes) {
        Some(Ok(bytes)) => {
            if let Some(dir) = skin_path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&skin_path, &bytes);
            bytes
        }
        _ => fs::read(&skin_path).context("no skin available")?,
    };
    let cape = fetch(cape_url, &cache_path(uuid, "-cape"));
    decode(&png, cape.as_deref())
}

pub fn cached(uuid: &str) -> Option<Skin> {
    let cape = fs::read(cache_path(uuid, "-cape")).ok();
    decode(&fs::read(cache_path(uuid, "")).ok()?, cape.as_deref()).ok()
}

pub fn forget(uuid: &str) {
    let _ = fs::remove_file(cache_path(uuid, ""));
    let _ = fs::remove_file(cache_path(uuid, "-cape"));
}

/// Slim skins leave the outer columns of each arm's texture empty (or, in
/// some editors, black). Checked on the parts only a 4 px arm would use.
fn looks_slim(texture: &[u8]) -> bool {
    let unused = |x: usize, y: usize, w: usize, h: usize| {
        (y..y + h).all(|py| {
            (x..x + w).all(|px| {
                let i = (py * 64 + px) * 4;
                let p = &texture[i..i + 4];
                p[3] == 0 || (p[0] == 0 && p[1] == 0 && p[2] == 0)
            })
        })
    };
    unused(50, 16, 2, 4) && unused(54, 20, 2, 12) && unused(42, 48, 2, 4) && unused(46, 52, 2, 12)
}

/// Copies a cape image (64x32, or an HD multiple) into the atlas below the skin.
fn add_cape(texture: &mut [u8], png: &[u8]) -> bool {
    let Ok(img) = image::load_from_memory_with_format(png, image::ImageFormat::Png) else { return false };
    let img = img.to_rgba8();
    let (w, h) = img.dimensions();
    // Capes are 64x32 (some older ones 64x32 padded from 22x17).
    if w < 64 || w % 64 != 0 || h < w / 2 {
        return false;
    }
    let scale = w / 64;
    for y in 0..32u32 {
        for x in 0..64u32 {
            let px = img.get_pixel(x * scale, y * scale).0;
            texture[(((64 + y) * 64 + x) * 4) as usize..][..4].copy_from_slice(&px);
        }
    }
    true
}

fn decode(png: &[u8], cape: Option<&[u8]>) -> Result<Skin> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)?.to_rgba8();
    let (w, h) = img.dimensions();
    if w < 64 || w % 64 != 0 || (h != w && h != w / 2) {
        bail!("not a skin image ({w}x{h})");
    }
    // HD skins are multiples of 64 px wide; sample them at the base scale.
    let scale = w / 64;
    let legacy = h == w / 2;
    let rows = if legacy { 32 } else { 64 };
    let mut texture = vec![0u8; ATLAS[0] * ATLAS[1] * 4];
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
    let slim = !legacy && looks_slim(&texture);
    let cape = cape.is_some_and(|png| add_cape(&mut texture, png));
    Ok(Skin { face, texture, legacy, slim, cape })
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
        let skin = decode(&png(&img), None).unwrap();
        assert_eq!(&skin.face[0..4], &[255, 0, 0, 255]);
        assert_eq!(&skin.face[4..8], &[0, 0, 255, 255]);
        assert!(!skin.legacy);
        assert_eq!(skin.texture.len(), 64 * 96 * 4);
    }

    #[test]
    fn legacy_and_hd_skins() {
        let legacy = image::RgbaImage::from_pixel(64, 32, image::Rgba([1, 2, 3, 255]));
        let s = decode(&png(&legacy), None).unwrap();
        assert!(s.legacy);
        // The missing bottom half stays transparent.
        assert_eq!(s.texture[(40 * 64 + 10) * 4 + 3], 0);

        let mut hd = image::RgbaImage::from_pixel(128, 128, image::Rgba([0, 0, 0, 0]));
        hd.put_pixel(16, 16, image::Rgba([9, 9, 9, 255]));
        assert_eq!(&decode(&png(&hd), None).unwrap().face[0..4], &[9, 9, 9, 255]);

        assert!(decode(&png(&image::RgbaImage::new(64, 48)), None).is_err());
    }

    #[test]
    fn detects_slim_arms_from_the_image() {
        // A classic skin uses the full 4 px of each arm.
        let classic = image::RgbaImage::from_pixel(64, 64, image::Rgba([90, 60, 40, 255]));
        assert!(!decode(&png(&classic), None).unwrap().slim);
        // A slim one leaves the outer columns empty.
        let mut slim = classic.clone();
        for (x, y, w, h) in [(50, 16, 2, 4), (54, 20, 2, 12), (42, 48, 2, 4), (46, 52, 2, 12)] {
            for py in y..y + h {
                for px in x..x + w {
                    slim.put_pixel(px, py, image::Rgba([0, 0, 0, 0]));
                }
            }
        }
        assert!(decode(&png(&slim), None).unwrap().slim);
    }

    #[test]
    fn cape_goes_below_the_skin() {
        let skin = image::RgbaImage::from_pixel(64, 64, image::Rgba([1, 1, 1, 255]));
        let mut cape = image::RgbaImage::from_pixel(64, 32, image::Rgba([0, 0, 0, 0]));
        cape.put_pixel(1, 1, image::Rgba([200, 0, 0, 255]));
        let s = decode(&png(&skin), Some(&png(&cape))).unwrap();
        assert!(s.cape);
        assert_eq!(&s.texture[((65 * 64 + 1) * 4)..][..4], &[200, 0, 0, 255]);
        assert!(!decode(&png(&skin), Some(b"not a png")).unwrap().cape);
    }

    #[test]
    fn cache_path_ignores_odd_characters() {
        let p = cache_path("../../evil", "");
        assert!(p.ends_with("skins/e.png") || p.ends_with("skins\\e.png"), "{p:?}");
    }
}

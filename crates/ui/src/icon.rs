//! Exile's icon, drawn in code: an accent-blue "E" on a rounded square in the
//! app background colour. Designed on a 32x32 grid and drawn at any size, so
//! the window icon and the installed app's icon file are the same picture.

/// The "E" as rectangles on the 32x32 grid: x from, x to, y from, y to.
const LETTER: [[f32; 4]; 4] = [
    [10.0, 13.0, 9.0, 23.0],  // upright
    [10.0, 22.0, 9.0, 12.0],  // top arm
    [10.0, 20.0, 15.0, 17.0], // middle arm
    [10.0, 22.0, 20.0, 23.0], // bottom arm
];
const BG: [f32; 3] = [0x11 as f32, 0x14 as f32, 0x19 as f32];
const FG: [f32; 3] = [0x67 as f32, 0x8f as f32, 0xc5 as f32];

/// RGBA pixels, `size` by `size`, with smoothed edges.
pub fn rgba(size: u32) -> Vec<u8> {
    const SAMPLES: u32 = 4;
    let scale = 32.0 / size as f32;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (mut square, mut letter) = (0u32, 0u32);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let gx = (x as f32 + (sx as f32 + 0.5) / SAMPLES as f32) * scale;
                    let gy = (y as f32 + (sy as f32 + 0.5) / SAMPLES as f32) * scale;
                    if in_square(gx, gy) {
                        square += 1;
                        if LETTER.iter().any(|r| (r[0]..r[1]).contains(&gx) && (r[2]..r[3]).contains(&gy)) {
                            letter += 1;
                        }
                    }
                }
            }
            let t = if square == 0 { 0.0 } else { letter as f32 / square as f32 };
            let mix = |i: usize| (BG[i] + (FG[i] - BG[i]) * t).round() as u8;
            let alpha = (square as f32 / (SAMPLES * SAMPLES) as f32 * 255.0).round() as u8;
            out.extend_from_slice(&[mix(0), mix(1), mix(2), alpha]);
        }
    }
    out
}

/// A Windows .ico with the icon at each size, every image stored as PNG
/// (`encode_png` turns RGBA pixels of the given size into a PNG file). The
/// build scripts use this to give the executables their icon.
#[allow(dead_code)]
pub fn ico(sizes: &[u32], encode_png: impl Fn(&[u8], u32) -> Vec<u8>) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| encode_png(&rgba(s), s)).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&size, png) in sizes.iter().zip(&images) {
        // Width and height are a byte each; 0 means 256.
        let side = if size >= 256 { 0 } else { size as u8 };
        out.extend_from_slice(&[side, side, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for png in images {
        out.extend_from_slice(&png);
    }
    out
}

/// The background: a square inset half a unit, with corners of radius 5.
fn in_square(x: f32, y: f32) -> bool {
    let (a, b) = ((x - 16.0).abs(), (y - 16.0).abs());
    if a > 15.5 || b > 15.5 {
        return false;
    }
    let (ca, cb) = ((a - 10.5).max(0.0), (b - 10.5).max(0.0));
    ca * ca + cb * cb <= 25.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_and_corners() {
        let px = |img: &[u8], size: u32, x: u32, y: u32| img[((y * size + x) * 4) as usize..][..4].to_vec();
        let img = rgba(32);
        assert_eq!(px(&img, 32, 0, 0)[3], 0, "corners are transparent");
        assert_eq!(px(&img, 32, 11, 16), [0x67, 0x8f, 0xc5, 0xff], "upright of the E");
        assert_eq!(px(&img, 32, 16, 13), [0x11, 0x14, 0x19, 0xff], "between the arms");
        let big = rgba(256);
        assert_eq!(big.len(), 256 * 256 * 4);
        assert_eq!(px(&big, 256, 88, 128), [0x67, 0x8f, 0xc5, 0xff]);
    }

    #[test]
    fn ico_directory() {
        // Stand-in "PNGs": one byte per side length.
        let file = ico(&[16, 256], |_, size| vec![0xAB; size as usize]);
        assert_eq!(&file[..6], &[0, 0, 1, 0, 2, 0]);
        assert_eq!(file[6], 16);
        assert_eq!(file[22], 0, "256 is written as 0");
        let offset = |entry: usize| u32::from_le_bytes(file[6 + 16 * entry + 12..][..4].try_into().unwrap()) as usize;
        assert_eq!(offset(0), 6 + 32);
        assert_eq!(offset(1), 6 + 32 + 16);
        assert_eq!(file.len(), 6 + 32 + 16 + 256);
    }
}

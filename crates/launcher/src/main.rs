#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod auth;
mod install;
mod instance;
mod java;
mod launch;
mod loader;
mod memory;
mod mods;
mod net;
mod paths;
mod secure_store;
mod skin;
mod theme;
mod versions;
mod widgets;

use eframe::egui;

/// 32x32 window icon drawn in code: an accent-blue "E" on the app background.
fn icon() -> egui::IconData {
    const N: usize = 32;
    let mut rgba = vec![0u8; N * N * 4];
    let bg = [0x11, 0x14, 0x19, 0xff];
    let fg = [0x67, 0x8f, 0xc5, 0xff];
    for y in 0..N {
        for x in 0..N {
            // Rounded square background.
            let (dx, dy) = ((x as i32 * 2 - 31).abs(), (y as i32 * 2 - 31).abs());
            let corner = (dx - 21).max(0).pow(2) + (dy - 21).max(0).pow(2) > 100;
            let in_e = (9..23).contains(&y)
                && ((10..13).contains(&x)
                    || ((9..12).contains(&y) || (20..23).contains(&y)) && (10..22).contains(&x)
                    || (15..17).contains(&y) && (10..20).contains(&x));
            let px = if corner { [0, 0, 0, 0] } else if in_e { fg } else { bg };
            rgba[(y * N + x) * 4..][..4].copy_from_slice(&px);
        }
    }
    egui::IconData { rgba, width: N as u32, height: N as u32 }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Exile")
            .with_app_id("exile-launcher")
            .with_inner_size([1040.0, 680.0])
            .with_min_inner_size([860.0, 560.0])
            .with_icon(icon()),
        ..Default::default()
    };
    eframe::run_native("Exile", options, Box::new(|cc| Ok(Box::new(app::ExileApp::new(cc)))))
}

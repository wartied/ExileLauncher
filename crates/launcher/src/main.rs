#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aot;
mod app;
mod auth;
mod badges;
mod changelog;
mod client;
mod game_jar;
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
mod skin3d;
mod update;
mod versions;

use eframe::egui;
// Shared with the installer; `crate::theme` and `crate::widgets` work as before.
use exile_ui::{theme, widgets};

fn icon() -> egui::IconData {
    egui::IconData { rgba: exile_ui::icon::rgba(32), width: 32, height: 32 }
}

fn main() -> eframe::Result {
    update::clean_up();
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

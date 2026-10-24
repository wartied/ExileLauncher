//! Launcher updates in the UI: the version label in the status bar, the
//! update setting, and the check/download/install flow (see `update`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText, Ui};

use super::{Event, ExileApp};
use crate::instance::UpdateMode;
use crate::net::{self, Progress};
use crate::theme::*;
use crate::update::{self, Release};
use crate::widgets;

/// How often a running launcher looks for updates again.
const RECHECK: Duration = Duration::from_secs(6 * 60 * 60);

pub(super) enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading { version: String, progress: Arc<Progress> },
    /// Downloaded and verified; installs on restart or when Exile closes.
    Ready { version: String, staged: PathBuf },
    Failed(String),
}

impl ExileApp {
    /// At start-up and every few hours, unless updates are off.
    pub(super) fn update_tick(&mut self) {
        if self.settings.updates == UpdateMode::Off {
            return;
        }
        let due = self.last_update_check.is_none_or(|t| t.elapsed() >= RECHECK);
        if due && matches!(self.update, UpdateState::Idle | UpdateState::UpToDate | UpdateState::Failed(_)) {
            self.check_for_update();
        }
    }

    fn check_for_update(&mut self) {
        self.last_update_check = Some(Instant::now());
        self.update = UpdateState::Checking;
        self.spawn(|| Event::UpdateChecked(update::check().map_err(|e| format!("{e:#}"))));
    }

    pub(super) fn on_update_checked(&mut self, result: Result<Option<Release>, String>) {
        self.update = match result {
            Ok(None) => UpdateState::UpToDate,
            Ok(Some(release)) if self.settings.updates == UpdateMode::Auto => {
                self.download_update(release);
                return;
            }
            Ok(Some(release)) => UpdateState::Available(release),
            Err(e) => UpdateState::Failed(e),
        };
    }

    fn download_update(&mut self, release: Release) {
        let progress = Arc::new(Progress::default());
        self.update = UpdateState::Downloading { version: release.version.clone(), progress: progress.clone() };
        self.spawn(move || {
            let result = update::download(&release, &progress).map(|staged| (release.version.clone(), staged));
            Event::UpdateDownloaded(result.map_err(|e| if net::is_cancelled(&e) { String::new() } else { format!("{e:#}") }))
        });
    }

    pub(super) fn on_update_downloaded(&mut self, result: Result<(String, PathBuf), String>) {
        self.update = match result {
            Ok((version, staged)) => UpdateState::Ready { version, staged },
            Err(e) if e.is_empty() => UpdateState::Idle,
            Err(e) => UpdateState::Failed(e),
        };
    }

    /// Installs a downloaded update when the launcher closes, so the next
    /// start runs it even if "Restart" was never pressed.
    pub(super) fn install_update_on_exit(&mut self) {
        if let UpdateState::Ready { staged, .. } = &self.update {
            let _ = update::install(staged);
        }
    }

    /// Restarting only ends the launcher (games keep running and are picked up
    /// again), but not in the middle of an install or download.
    fn busy(&self) -> bool {
        !self.tasks.is_empty() || !self.mod_jobs.is_empty()
    }

    /// Right-hand end of the status bar: the version, and the update state
    /// when there is one. Laid out right to left.
    pub(super) fn update_status(&mut self, ui: &mut Ui) {
        let version = ui
            .add(egui::Label::new(RichText::new(format!("Exile {}", update::CURRENT)).size(12.0).color(TEXT_FAINT)).sense(egui::Sense::click()))
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        egui::Popup::menu(&version)
            .align(egui::RectAlign::TOP_END)
            .gap(8.0)
            .width(300.0)
            .show(|ui| self.update_menu(ui));

        let link = |ui: &mut Ui, text: String, color: Color32| {
            ui.add(egui::Label::new(RichText::new(text).size(12.0).color(color)).sense(egui::Sense::click()))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
        };
        match &self.update {
            UpdateState::Available(release) => {
                let release = release.clone();
                ui.add_space(10.0);
                if link(ui, format!("Update to {}", release.version), ACCENT).clicked() {
                    self.download_update(release);
                }
            }
            UpdateState::Downloading { version, progress } => {
                ui.add_space(10.0);
                ui.label(RichText::new(format!("Downloading {version} · {:.0}%", progress.fraction() * 100.0)).size(12.0).color(TEXT_DIM));
            }
            UpdateState::Ready { version, staged } => {
                ui.add_space(10.0);
                if self.busy() {
                    ui.label(RichText::new(format!("Exile {version} installs when you close Exile")).size(12.0).color(TEXT_DIM));
                } else if link(ui, format!("Restart to update to {version}"), ACCENT).clicked() {
                    match update::install(staged) {
                        Ok(()) => update::restart(),
                        Err(e) => self.update = UpdateState::Failed(format!("{e:#}")),
                    }
                }
            }
            UpdateState::Failed(e) if self.settings.updates != UpdateMode::Off => {
                ui.add_space(10.0);
                ui.label(RichText::new("Update failed").size(12.0).color(TEXT_FAINT)).on_hover_text(e.as_str());
            }
            _ => {}
        }
    }

    fn update_menu(&mut self, ui: &mut Ui) {
        // Menus lay out justified, which stretches wrapped text word by word.
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| self.update_menu_contents(ui));
    }

    fn update_menu_contents(&mut self, ui: &mut Ui) {
        ui.set_min_width(280.0);
        ui.add_space(4.0);
        ui.label(RichText::new(format!("Exile {}", update::CURRENT)).family(semibold()));
        ui.label(RichText::new("Launcher and Exile Client update together.").size(12.0).color(TEXT_DIM));
        ui.add_space(10.0);
        ui.label(RichText::new("Updates").size(12.0).color(TEXT_DIM));
        ui.add_space(4.0);
        let modes = [UpdateMode::Auto, UpdateMode::Ask, UpdateMode::Off];
        let active = modes.iter().position(|m| *m == self.settings.updates).unwrap_or(1);
        if let Some(i) = widgets::segmented(ui, &[("Automatic", true), ("Ask me", true), ("Off", true)], active, "") {
            self.settings.updates = modes[i];
            self.settings.save();
            if self.settings.updates != UpdateMode::Off && matches!(self.update, UpdateState::Idle | UpdateState::Failed(_)) {
                self.check_for_update();
            }
        }
        ui.add_space(6.0);
        let hint = match self.settings.updates {
            UpdateMode::Auto => "Downloads updates and installs them when you close Exile.",
            UpdateMode::Ask => "Shows when an update is available; you choose when.",
            UpdateMode::Off => "Never checks. Get new versions from exiles.dev.",
        };
        ui.label(RichText::new(hint).size(12.0).color(TEXT_DIM));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let checking = matches!(self.update, UpdateState::Checking | UpdateState::Downloading { .. });
            if widgets::button_enabled(ui, !checking, "Check now").clicked() {
                self.check_for_update();
            }
            let status = match &self.update {
                UpdateState::Checking => "Checking…".to_owned(),
                UpdateState::UpToDate => "You're up to date.".to_owned(),
                UpdateState::Available(r) => format!("{} is available.", r.version),
                UpdateState::Downloading { version, progress } => format!("Downloading {version}: {:.0}%", progress.fraction() * 100.0),
                UpdateState::Ready { version, .. } => format!("{version} is ready."),
                UpdateState::Failed(_) => "Update failed.".to_owned(),
                UpdateState::Idle => String::new(),
            };
            ui.label(RichText::new(status).size(12.0).color(TEXT_DIM));
        });
        match &self.update {
            UpdateState::Available(r) if !r.notes.trim().is_empty() => {
                ui.add_space(8.0);
                ui.label(RichText::new(r.notes.trim()).size(12.0).color(TEXT_DIM));
            }
            UpdateState::Failed(e) => {
                ui.add_space(8.0);
                ui.label(RichText::new(e.as_str()).size(12.0).color(TEXT_DIM));
            }
            _ => {}
        }
        ui.add_space(4.0);
    }

    /// True while something on screen is moving (the download percentage).
    pub(super) fn update_animating(&self) -> bool {
        matches!(self.update, UpdateState::Downloading { .. })
    }
}

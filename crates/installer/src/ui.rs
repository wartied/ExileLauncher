//! The installer's window, in the launcher's look: the EXILE bar on top, one
//! step per page, and the page's buttons in a footer.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Align, CornerRadius, Frame, Layout, Margin, RichText, Stroke, Ui, ViewportCommand, vec2};
use exile_ui::theme::{self, *};
use exile_ui::widgets::{self, Kind};

use crate::payload::Payload;
use crate::setup::{self, VERSION};
use crate::system::{self, Places};

pub enum Mode {
    Install { me: PathBuf, payload: Payload },
    Uninstall,
}

enum Page {
    Start,
    Working,
    Done,
    Failed(String),
}

#[derive(Default)]
struct Progress {
    fraction: f32,
    step: String,
    result: Option<Result<(), String>>,
}

pub struct Setup {
    mode: Mode,
    places: Places,
    /// The version in Apps, or "" when exile.exe is there without an entry.
    installed: Option<String>,
    page: Page,
    desktop: bool,
    open_after: bool,
    delete_data: bool,
    progress: Arc<Mutex<Progress>>,
}

pub fn run(mode: Mode, places: Places) -> eframe::Result {
    let title = match mode {
        Mode::Install { .. } => "Exile Setup",
        Mode::Uninstall => "Uninstall Exile",
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([600.0, 440.0])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_icon(egui::IconData { rgba: exile_ui::icon::rgba(32), width: 32, height: 32 }),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        title,
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            theme::set_dark(&cc.egui_ctx, uses_dark_theme(&places));
            Ok(Box::new(Setup::new(mode, places)))
        }),
    )
}

/// Matches the theme picked in the launcher, if Exile has been used here.
fn uses_dark_theme(places: &Places) -> bool {
    std::fs::read_to_string(places.data.join("settings.toml"))
        .is_ok_and(|s| s.lines().any(|l| l.replace(' ', "") == "dark_theme=true"))
}

impl Setup {
    fn new(mode: Mode, places: Places) -> Self {
        let installed = system::installed_version(&places).or_else(|| places.exe().exists().then(String::new));
        // On a reinstall, keep the desktop shortcut choice made last time.
        let desktop = installed.is_none() || places.desktop_link().exists();
        Self {
            mode,
            places,
            installed,
            page: Page::Start,
            desktop,
            open_after: true,
            delete_data: false,
            progress: Default::default(),
        }
    }

    fn installing(&self) -> bool {
        matches!(self.mode, Mode::Install { .. })
    }

    fn start(&mut self, ctx: &egui::Context) {
        self.page = Page::Working;
        *self.progress.lock().unwrap() = Progress::default();
        let places = self.places.clone();
        let job: Box<dyn FnOnce(setup::Report) -> anyhow::Result<()> + Send> = match &self.mode {
            Mode::Install { me, payload } => {
                let (me, payload, desktop) = (me.clone(), payload.clone(), self.desktop);
                Box::new(move |report| setup::install(&places, &me, &payload, desktop, report))
            }
            Mode::Uninstall => {
                let delete_data = self.delete_data;
                Box::new(move |report| setup::uninstall(&places, delete_data, report))
            }
        };
        let (progress, ctx) = (self.progress.clone(), ctx.clone());
        std::thread::spawn(move || {
            let report = |fraction: f32, step: &str| {
                let mut p = progress.lock().unwrap();
                p.fraction = fraction;
                p.step = step.to_owned();
                ctx.request_repaint();
            };
            let result = job(&report).map_err(|e| sentence(&format!("{e:#}")));
            progress.lock().unwrap().result = Some(result);
            ctx.request_repaint();
        });
    }

    fn finish(&self, ctx: &egui::Context) {
        if self.installing() && self.open_after {
            let _ = std::process::Command::new(self.places.exe()).current_dir(&self.places.dir).spawn();
        }
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    // ---------------------------------------------------------------- pages

    fn page(&mut self, ui: &mut Ui) {
        match &self.page {
            Page::Start if self.installing() => self.install_start(ui),
            Page::Start => self.uninstall_start(ui),
            Page::Working => self.working(ui),
            Page::Done => self.done(ui),
            Page::Failed(error) => {
                let error = error.clone();
                title(ui, if self.installing() { "Couldn't install Exile" } else { "Couldn't uninstall Exile" });
                body(ui, &error);
            }
        }
    }

    fn install_start(&mut self, ui: &mut Ui) {
        match self.installed.as_deref() {
            None => {
                title(ui, "Install Exile");
                body(ui, "A fast, native Minecraft launcher. It installs for your Windows account only, so no administrator is needed.");
            }
            Some(old) => {
                let newer = !old.is_empty() && newer_than(old, VERSION);
                title(ui, if newer || old == VERSION || old.is_empty() { "Reinstall Exile" } else { "Update Exile" });
                let installed = if old.is_empty() { "Exile".to_owned() } else { format!("Exile {old}") };
                body(
                    ui,
                    &format!(
                        "{installed} is already installed. This puts Exile {VERSION} in its place. Your instances, worlds and settings stay as they are."
                    ),
                );
            }
        }
        ui.add_space(22.0);
        row(ui, "Location", &self.places.dir.display().to_string(), |_| {});
        row(ui, "Desktop shortcut", "Exile is always added to the Start menu.", |ui| {
            widgets::toggle(ui, &mut self.desktop, true);
        });
    }

    fn uninstall_start(&mut self, ui: &mut Ui) {
        if self.installed.is_none() {
            title(ui, "Exile isn't installed");
            body(ui, "There's nothing to uninstall.");
            return;
        }
        title(ui, "Uninstall Exile");
        body(ui, "Removes Exile, its shortcuts and its entry in Apps.");
        ui.add_space(22.0);
        let detail = format!("Instances, worlds, screenshots and settings in {}. This can't be undone.", self.places.data.display());
        row(ui, "Also delete my data", &detail, |ui| {
            widgets::toggle(ui, &mut self.delete_data, true);
        });
    }

    fn working(&mut self, ui: &mut Ui) {
        title(ui, if self.installing() { "Installing Exile" } else { "Uninstalling Exile" });
        let (fraction, step, result) = {
            let p = self.progress.lock().unwrap();
            (p.fraction, p.step.clone(), p.result.clone())
        };
        ui.add_space(26.0);
        let shown = ui.ctx().animate_value_with_time(ui.scope_id().with("progress"), fraction, 0.25);
        progress_bar(ui, shown);
        ui.add_space(10.0);
        ui.label(RichText::new(step).color(text_dim()));
        match result {
            // Let the bar reach the end before moving on.
            Some(Ok(())) if shown >= 0.999 => self.page = Page::Done,
            Some(Err(e)) => self.page = Page::Failed(e),
            _ => {}
        }
    }

    fn done(&mut self, ui: &mut Ui) {
        if self.installing() {
            title(ui, "Exile is installed");
            let where_ = if self.desktop { "the Start menu or your desktop" } else { "the Start menu" };
            body(ui, &format!("Open it from {where_}. Exile keeps itself up to date from now on."));
            ui.add_space(22.0);
            row(ui, "Open Exile now", "", |ui| {
                widgets::toggle(ui, &mut self.open_after, true);
            });
        } else {
            title(ui, "Exile is uninstalled");
            if self.places.data.exists() {
                body(
                    ui,
                    &format!(
                        "Your instances, worlds and settings are still in {}, in case you come back.",
                        self.places.data.display()
                    ),
                );
            } else {
                body(ui, "Thanks for playing.");
            }
        }
    }

    fn footer(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new(format!("Exile {VERSION}")).size(12.0).color(text_faint()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let size = vec2(120.0, 36.0);
                match &self.page {
                    Page::Start if self.installing() => {
                        let label = if self.installed.is_some() { "Reinstall" } else { "Install" };
                        if widgets::filled_button(ui, label, size, Kind::Primary, true).clicked() {
                            self.start(ui.ctx());
                        }
                        if widgets::outline_button(ui, "Cancel", size, true, text()).clicked() {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                        }
                    }
                    Page::Start if self.installed.is_some() => {
                        if widgets::filled_button(ui, "Uninstall", size, Kind::Danger, true).clicked() {
                            self.start(ui.ctx());
                        }
                        if widgets::outline_button(ui, "Cancel", size, true, text()).clicked() {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                        }
                    }
                    Page::Start => {
                        if widgets::outline_button(ui, "Close", size, true, text()).clicked() {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                        }
                    }
                    Page::Working => {}
                    Page::Done => {
                        let label = if self.installing() { "Finish" } else { "Close" };
                        if widgets::filled_button(ui, label, size, Kind::Primary, true).clicked() {
                            self.finish(ui.ctx());
                        }
                    }
                    Page::Failed(_) => {
                        if widgets::filled_button(ui, "Try again", size, Kind::Primary, true).clicked() {
                            self.start(ui.ctx());
                        }
                        if widgets::outline_button(ui, "Close", size, true, text()).clicked() {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                        }
                    }
                }
            });
        });
    }
}

impl eframe::App for Setup {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Closing halfway would leave a half-installed Exile.
        if matches!(self.page, Page::Working) && ui.ctx().input(|i| i.viewport().close_requested()) {
            ui.ctx().send_viewport_cmd(ViewportCommand::CancelClose);
        }

        egui::Panel::top("top")
            .exact_size(56.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(sidebar()).inner_margin(Margin::symmetric(32, 0)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("EXILE").size(15.0).family(theme::semibold()).color(text()).extra_letter_spacing(3.0));
                });
                let r = ui.max_rect();
                ui.painter().hline(ui.clip_rect().x_range(), r.bottom() - 0.5, Stroke::new(1.0, border()));
            });

        egui::Panel::bottom("footer")
            .exact_size(68.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(sidebar()).inner_margin(Margin::symmetric(32, 0)))
            .show(ui, |ui| {
                let r = ui.max_rect();
                ui.painter().hline(ui.clip_rect().x_range(), r.top() + 0.5, Stroke::new(1.0, border()));
                self.footer(ui);
            });

        egui::CentralPanel::default()
            .frame(Frame::new().fill(bg()).inner_margin(Margin { left: 32, right: 32, top: 30, bottom: 20 }))
            .show(ui, |ui| self.page(ui));
    }
}

fn title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(26.0).family(theme::semibold()).color(theme::text()));
    ui.add_space(6.0);
}

fn body(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).color(text_dim())).wrap());
}

/// A title and explanation on the left, its control on the right, like a
/// row in the launcher's settings.
fn row(ui: &mut Ui, title: &str, detail: &str, control: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), Layout::right_to_left(Align::Min), |ui| {
        control(ui);
        ui.add_space(24.0);
        ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), Layout::top_down(Align::Min), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(title).color(theme::text()));
            if !detail.is_empty() {
                ui.label(RichText::new(detail).size(12.0).color(text_faint()));
            }
        });
    });
    ui.add_space(16.0);
}

fn progress_bar(ui: &mut Ui, fraction: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(3), surface_active());
    let mut done = rect;
    done.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    if done.width() > 0.5 {
        ui.painter().rect_filled(done, CornerRadius::same(3), ACCENT);
    }
}

/// "adding x: it failed" -> "Adding x: it failed."
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    let mut out: String = chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
    if !out.ends_with(['.', ')']) {
        out.push('.');
    }
    out
}

/// Whether version `a` is newer than `b` ("0.2.10" > "0.2.9").
fn newer_than(a: &str, b: &str) -> bool {
    let parse = |v: &str| v.split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parse(a) > parse(b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn versions_compare_by_number() {
        assert!(super::newer_than("0.2.10", "0.2.9"));
        assert!(!super::newer_than("0.2.2", "0.2.2"));
        assert!(!super::newer_than("0.2.1", "0.3.0"));
    }
}

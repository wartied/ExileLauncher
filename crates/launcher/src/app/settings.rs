//! The Settings page: Account (switcher, 3D skin, profile), Appearance,
//! Games (memory) and Launcher. It takes the place of the instance list and
//! details; Back returns to them.

use std::time::Duration;

use eframe::egui::{self, Align, Color32, CornerRadius, Layout, RichText, ScrollArea, Sense, Stroke, Ui, vec2};

use super::{Account, Event, ExileApp, Page};
use crate::auth::Session;
use crate::changelog::{Note, Release};
use crate::instance::UpdateMode;
use crate::memory::{self, fmt_mb};
use crate::theme::{self, *};
use crate::widgets;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Account,
    Appearance,
    Games,
    Launcher,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Account, Tab::Appearance, Tab::Games, Tab::Launcher];

    fn title(self) -> &'static str {
        match self {
            Tab::Account => "Account",
            Tab::Appearance => "Appearance",
            Tab::Games => "Games",
            Tab::Launcher => "Launcher",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Tab::Account => "Profile, skin and switching",
            Tab::Appearance => "Theme and size",
            Tab::Games => "Memory and running games",
            Tab::Launcher => "Updates and behaviour",
        }
    }
}

/// Release notes in Settings > Launcher, loaded the first time it opens.
#[derive(Default)]
pub struct Changelog {
    releases: Vec<Release>,
    requested: bool,
    loading: bool,
    error: Option<String>,
    /// The release whose notes are showing.
    open: Option<String>,
}

impl Changelog {
    pub(super) fn finish(&mut self, result: Result<Vec<Release>, String>) {
        self.loading = false;
        match result {
            Ok(releases) => {
                self.releases = releases;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
}

const UI_SCALES: [(&str, f32); 5] = [("90%", 0.9), ("100%", 1.0), ("110%", 1.1), ("125%", 1.25), ("150%", 1.5)];
const RUNNING_LIMITS: [(&str, Option<u32>); 5] = [("1", Some(1)), ("2", Some(2)), ("3", Some(3)), ("4", Some(4)), ("Any", None)];

impl ExileApp {
    pub(super) fn open_settings(&mut self, tab: Tab) {
        if tab == Tab::Account && self.page != Page::Settings(Tab::Account) {
            self.skin_view.restart();
        }
        self.page = Page::Settings(tab);
    }

    pub(super) fn settings_sidebar(&mut self, ui: &mut Ui, active: Tab) {
        egui::Panel::bottom("settings-footer")
            .show_separator_line(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 0, top: 10, bottom: 0 }))
            .show(ui, |ui| {
                let w = ui.available_width();
                if widgets::outline_button(ui, "Back to instances", vec2(w, 36.0), true, text()).clicked() {
                    self.page = Page::Instances;
                }
            });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            widgets::section_label(ui, "Settings");
        });
        ui.add_space(2.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        for tab in Tab::ALL {
            if widgets::instance_row(ui, tab == active, tab.title(), tab.subtitle(), None).clicked() {
                self.open_settings(tab);
            }
        }
    }

    pub(super) fn settings_page(&mut self, ui: &mut Ui, tab: Tab) {
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(680.0));
            ui.label(RichText::new(tab.title()).size(26.0).family(theme::semibold()).color(text()));
            ui.add_space(20.0);
            match tab {
                Tab::Account => self.account_tab(ui),
                Tab::Appearance => self.appearance_tab(ui),
                Tab::Games => self.games_tab(ui),
                Tab::Launcher => self.launcher_tab(ui),
            }
            ui.add_space(16.0);
        });
    }

    // ------------------------------------------------------------- account

    fn account_tab(&mut self, ui: &mut Ui) {
        let active = self.session().cloned();
        let signing_in = matches!(self.account, Account::SigningIn { .. });

        widgets::section_label(ui, "Accounts");
        ui.add_space(6.0);
        let mut switch_to = None;
        let mut add = false;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            if let Some(s) = &active {
                self.account_switch_chip(ui, s, true);
            }
            for other in self.others.clone() {
                if self.account_switch_chip(ui, &other, false).clicked() && !signing_in {
                    switch_to = Some(other.uuid.clone());
                }
            }
            let label = if signing_in { "Signing in…" } else { "+  Add account" };
            let width = widgets::text_width(ui, label, &egui::FontId::proportional(14.0)) + 32.0;
            if widgets::outline_button(ui, label, vec2(width, 40.0), !signing_in, text_dim())
                .clicked()
            {
                add = true;
            }
        });
        if let Some(uuid) = switch_to {
            self.switch_account(&uuid);
        }
        if add {
            self.sign_in();
        }
        if signing_in {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(14.0).color(text_dim()));
                ui.label(RichText::new("Finish signing in in your browser. Pick the account to add.").color(text_dim()));
                if ui.link("Cancel").clicked() {
                    self.cancel_sign_in();
                }
            });
        }
        ui.add_space(14.0);

        let Some(session) = active else {
            widgets::card(ui, |ui| {
                ui.label(RichText::new("No account").family(theme::semibold()).color(text()));
                ui.label(RichText::new("Sign in with Microsoft to play and to see your profile here.").color(text_dim()));
                ui.add_space(8.0);
                if !signing_in && widgets::button(ui, "Sign in with Microsoft").clicked() {
                    self.sign_in();
                }
            });
            return;
        };

        widgets::card(ui, |ui| {
            ui.horizontal_top(|ui| {
                self.skin_canvas(ui, &session);
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new(&session.name).size(22.0).family(theme::semibold()).color(text()));
                    ui.label(RichText::new("Minecraft: Java Edition").color(text_dim()));
                    ui.add_space(14.0);
                    let uuid = dashed_uuid(&session.uuid);
                    detail(ui, "UUID", &uuid, Some(&session.uuid));
                    let slim = session.skin_slim || self.skins.get(&session.uuid).is_some_and(|s| s.slim);
                    detail(ui, "Skin model", if slim { "Slim (Alex)" } else { "Classic (Steve)" }, None);
                    detail(ui, "Cape", session.cape.as_deref().unwrap_or("None"), None);
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if widgets::button(ui, "Change skin").clicked() {
                            let _ = open::that("https://www.minecraft.net/msaprofile/mygames/editskin");
                        }
                        if widgets::outline_button(ui, "Sign out", vec2(96.0, 30.0), true, DANGER).clicked() {
                            self.sign_out();
                        }
                    });
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Skin and cape changes show here the next time Exile starts.").size(12.0).color(text_faint()),
                    );
                });
            });
        });
    }

    /// A face and name in the switcher. The active account is highlighted.
    fn account_switch_chip(&self, ui: &mut Ui, session: &Session, active: bool) -> egui::Response {
        let font = egui::FontId::new(14.0, theme::semibold());
        let name_w = ui.fonts_mut(|f| f.layout_no_wrap(session.name.clone(), font.clone(), text()).size().x);
        let (rect, response) = ui.allocate_exact_size(vec2(10.0 + 24.0 + 10.0 + name_w + 14.0, 40.0), Sense::click());
        let hovered = response.hovered() && !active;
        let fill = if active { surface_active() } else if hovered { surface_hover() } else { surface() };
        let stroke = if active { Stroke::new(1.0, ACCENT) } else { Stroke::new(1.0, border()) };
        ui.painter().rect(rect.shrink(0.5), CornerRadius::same(RADIUS), fill, stroke, egui::StrokeKind::Inside);
        let face = egui::Rect::from_min_size(rect.min + vec2(10.0, 8.0), vec2(24.0, 24.0));
        widgets::paint_avatar(ui, face, self.face(&session.uuid), &session.name);
        widgets::paint_text(ui, face.right() + 10.0, Align::Min, rect.center().y, &session.name, font, if active { text() } else { text_dim() });
        if active {
            response
        } else {
            response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Switch to this account")
        }
    }

    /// The 3D model with a name tag above it. Drag sideways to turn it.
    fn skin_canvas(&mut self, ui: &mut Ui, session: &Session) {
        let (rect, response) = ui.allocate_exact_size(vec2(240.0, 320.0), Sense::drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(6), bg());
        if response.dragged() {
            self.skin_view.drag(response.drag_delta().x);
        }
        let cursor = if response.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab };
        response.on_hover_cursor(cursor);
        let model = rect.shrink2(vec2(10.0, 30.0)).translate(vec2(0.0, 16.0));
        let skin = self.skins.get(&session.uuid);
        crate::skin3d::paint(
            &painter,
            model,
            skin.map(|s| s.full.id()),
            session.skin_slim || skin.is_some_and(|s| s.slim),
            skin.is_some_and(|s| s.legacy),
            skin.is_some_and(|s| s.cape),
            &self.skin_view.pose(),
        );
        // A name tag, like the one over players in game.
        let font = egui::FontId::new(13.0, theme::semibold());
        let galley = painter.layout_no_wrap(session.name.clone(), font, Color32::WHITE);
        let tag = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 26.0), galley.size() + vec2(12.0, 6.0));
        painter.rect_filled(tag, CornerRadius::same(3), Color32::from_black_alpha(110));
        painter.galley(tag.min + vec2(6.0, 3.0), galley, Color32::WHITE);
        if self.skin_view.animating() {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
    }

    // ---------------------------------------------------------- appearance

    fn appearance_tab(&mut self, ui: &mut Ui) {
        setting(ui, "Theme", "Exile's blue-tinted greys, or neutral dark greys.", |ui| {
            let active = self.settings.dark_theme as usize;
            if let Some(i) = widgets::segmented(ui, &[("Exile", true), ("Dark", true)], active, "") {
                self.settings.dark_theme = i == 1;
                theme::set_dark(ui.ctx(), self.settings.dark_theme);
                self.settings.save();
            }
        });
        setting(ui, "Interface size", "Makes everything in the launcher larger or smaller.", |ui| {
            let active = UI_SCALES.iter().position(|(_, s)| (*s - self.settings.ui_scale).abs() < 0.01).unwrap_or(1);
            let options: Vec<(&str, bool)> = UI_SCALES.iter().map(|(l, _)| (*l, true)).collect();
            if let Some(i) = widgets::segmented(ui, &options, active, "") {
                self.settings.ui_scale = UI_SCALES[i].1;
                self.apply_ui_scale(self.settings.ui_scale);
                self.settings.save();
            }
        });
    }

    // --------------------------------------------------------------- games

    fn games_tab(&mut self, ui: &mut Ui) {
        let machine_max = memory::machine_max_mb(self.sys.total_mb).max(1024);
        let reserve = memory::os_reserve_mb(self.sys.total_mb);
        ui.label(
            RichText::new(format!(
                "This computer has {} of memory. Exile always leaves {} for your system and never starts more games than fit.",
                fmt_mb(self.sys.total_mb),
                fmt_mb(reserve)
            ))
            .color(text_dim()),
        );
        ui.add_space(14.0);

        let mut changed = false;
        setting(ui, "Memory for new instances", "Recommended picks the right amount for each Minecraft version.", |ui| {
            let mut custom = self.settings.default_memory_mb.is_some();
            if let Some(i) = widgets::segmented(ui, &[("Recommended", true), ("Custom", true)], custom as usize, "") {
                custom = i == 1;
                self.settings.default_memory_mb = custom.then_some(self.settings.default_memory_mb.unwrap_or(4096).min(machine_max));
                changed = true;
            }
        });
        if let Some(mb) = self.settings.default_memory_mb {
            changed |= memory_slider(ui, mb, 1024, machine_max, |v| self.settings.default_memory_mb = Some(v));
            ui.add_space(4.0);
            ui.label(RichText::new("Versions that need more still get at least their minimum.").size(12.0).color(text_faint()));
            ui.add_space(12.0);
        }

        setting(ui, "Maximum per instance", "Lowers the most any one game may use. It can't go above what's safe.", |ui| {
            let mut limited = self.settings.max_memory_mb.is_some();
            if let Some(i) = widgets::segmented(ui, &[("Safe maximum", true), ("Custom", true)], limited as usize, "") {
                limited = i == 1;
                self.settings.max_memory_mb = limited.then_some(self.settings.max_memory_mb.unwrap_or(machine_max.min(8192)));
                changed = true;
            }
        });
        if let Some(mb) = self.settings.max_memory_mb {
            changed |= memory_slider(ui, mb, 1024, machine_max, |v| self.settings.max_memory_mb = Some(v));
            ui.add_space(12.0);
        }

        setting(ui, "Games running at once", "Each one still has to fit in memory.", |ui| {
            let active = RUNNING_LIMITS.iter().position(|(_, n)| *n == self.settings.max_running).unwrap_or(4);
            let options: Vec<(&str, bool)> = RUNNING_LIMITS.iter().map(|(l, _)| (*l, true)).collect();
            if let Some(i) = widgets::segmented(ui, &options, active, "") {
                self.settings.max_running = RUNNING_LIMITS[i].1;
                changed = true;
            }
        });
        if changed {
            self.settings.save();
        }
    }

    // ------------------------------------------------------------ launcher

    fn launcher_tab(&mut self, ui: &mut Ui) {
        setting(ui, "Updates", "Exile and Exile Client update together. Every update is signed.", |ui| {
            let modes = [UpdateMode::Auto, UpdateMode::Ask, UpdateMode::Off];
            let active = modes.iter().position(|m| *m == self.settings.updates).unwrap_or(1);
            if let Some(i) = widgets::segmented(ui, &[("Automatic", true), ("Ask me", true), ("Off", true)], active, "") {
                self.set_update_mode(modes[i]);
            }
        });
        ui.horizontal(|ui| {
            if widgets::button(ui, "Check now").clicked() {
                self.check_for_update_now();
            }
            ui.label(RichText::new(self.update_summary()).color(text_dim()));
        });
        ui.add_space(18.0);

        let mut changed = false;
        setting(ui, "Minimise while playing", "Moves the launcher out of the way when a game starts.", |ui| {
            changed |= widgets::toggle(ui, &mut self.settings.minimize_on_launch, true).changed();
        });
        setting(ui, "Show snapshots", "Lists test versions of Minecraft when you create an instance.", |ui| {
            changed |= widgets::toggle(ui, &mut self.settings.show_snapshots, true).changed();
        });
        if changed {
            self.settings.save();
        }
        setting(ui, "Data folder", &crate::paths::root().display().to_string(), |ui| {
            if widgets::button(ui, "Open").clicked() {
                let _ = open::that(crate::paths::root());
            }
        });
        ui.add_space(8.0);
        self.changelog(ui);
    }

    fn load_changelog(&mut self) {
        self.changelog.requested = true;
        self.changelog.loading = true;
        self.spawn(|| Event::Changelog(crate::changelog::load().map_err(|e| format!("{e:#}"))));
    }

    /// Every release, newest first. Clicking one shows its notes under it.
    fn changelog(&mut self, ui: &mut Ui) {
        if !self.changelog.requested {
            // The saved copy shows straight away while the list refreshes.
            self.changelog.releases = crate::changelog::cached().unwrap_or_default();
            self.changelog.open = Some(crate::update::CURRENT.to_owned());
            self.load_changelog();
        }
        widgets::section_label(ui, "Changelog");
        ui.add_space(6.0);

        if self.changelog.releases.is_empty() {
            ui.horizontal(|ui| {
                if self.changelog.loading {
                    ui.add(egui::Spinner::new().size(14.0).color(text_dim()));
                    ui.label(RichText::new("Loading release notes…").color(text_dim()));
                } else {
                    ui.label(RichText::new("Couldn't load the release notes.").color(text_dim()));
                    if ui.link("Try again").clicked() {
                        self.load_changelog();
                    }
                }
            });
            return;
        }

        let mut toggled = None;
        egui::Frame::new()
            .fill(surface())
            .stroke(Stroke::new(1.0, border()))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ScrollArea::vertical().id_salt("changelog").max_height(300.0).min_scrolled_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, release) in self.changelog.releases.iter().enumerate() {
                        if i > 0 {
                            let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                            ui.painter().hline(line.x_range().shrink(10.0), line.center().y, Stroke::new(1.0, border()));
                        }
                        let open = self.changelog.open.as_deref() == Some(&release.version);
                        if release_row(ui, release, open).clicked() {
                            toggled = Some(release.version.clone());
                        }
                        if open {
                            release_notes(ui, &release.notes);
                        }
                    }
                });
            });
        if let Some(version) = toggled {
            self.changelog.open = if self.changelog.open.as_ref() == Some(&version) { None } else { Some(version) };
        }
    }
}

/// A release in the changelog: version, "Installed" for the running one,
/// its date and a chevron that points down while its notes show.
fn release_row(ui: &mut Ui, release: &Release, open: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect.shrink(1.0), CornerRadius::same(5), surface_hover());
        }
        let y = rect.center().y;
        let font = egui::FontId::new(14.0, theme::semibold());
        let name = format!("Exile {}", release.version);
        let name_w = widgets::text_width(ui, &name, &font);
        widgets::paint_text(ui, rect.left() + 12.0, Align::Min, y, &name, font, text());
        if release.version == crate::update::CURRENT {
            let small = egui::FontId::proportional(11.0);
            let w = widgets::text_width(ui, "Installed", &small) + 12.0;
            let pill = egui::Rect::from_min_size(egui::pos2(rect.left() + 12.0 + name_w + 10.0, y - 9.0), vec2(w, 18.0));
            ui.painter().rect_filled(pill, CornerRadius::same(9), ACCENT_DIM);
            widgets::paint_text(ui, pill.center().x, Align::Center, y, "Installed", small, ACCENT_LIGHT);
        }

        // Chevron: right when closed, down when open.
        let c = egui::pos2(rect.right() - 18.0, y);
        let stroke = Stroke::new(1.5, if response.hovered() { text_dim() } else { text_faint() });
        let points = if open {
            [c + vec2(-4.0, -2.0), c + vec2(0.0, 2.0), c + vec2(4.0, -2.0)]
        } else {
            [c + vec2(-2.0, -4.0), c + vec2(2.0, 0.0), c + vec2(-2.0, 4.0)]
        };
        ui.painter().line(points.to_vec(), stroke);
        widgets::paint_text(ui, rect.right() - 34.0, Align::Max, y, &release.date, egui::FontId::proportional(12.0), text_faint());
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn release_notes(ui: &mut Ui, notes: &[Note]) {
    egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 16, top: 0, bottom: 12 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 4.0;
        for note in notes {
            match note {
                Note::Bullet(line) => {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.label(RichText::new("•").color(text_faint()));
                        ui.add(egui::Label::new(RichText::new(line).color(text_dim())).wrap());
                    });
                }
                Note::Text(line) => {
                    ui.add(egui::Label::new(RichText::new(line).color(text_dim())).wrap());
                }
            }
        }
    });
}

/// A setting: title and explanation on the left, its control on the right.
/// The control is laid out first, so it always keeps its full size, and the
/// explanation wraps into the width left. On a narrow page (a large
/// interface size) the control goes under the text instead.
fn setting(ui: &mut Ui, title: &str, detail: &str, control: impl FnOnce(&mut Ui)) {
    let labels = |ui: &mut Ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(RichText::new(title).color(text()));
        ui.label(RichText::new(detail).size(12.0).color(text_faint()));
    };
    if ui.available_width() < 560.0 {
        labels(ui);
        ui.add_space(6.0);
        ui.horizontal(control);
    } else {
        ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), Layout::right_to_left(Align::Min), |ui| {
            control(ui);
            ui.add_space(24.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), Layout::top_down(Align::Min), labels);
        });
    }
    ui.add_space(16.0);
}

/// A memory slider with its value, in the same style as an instance's.
fn memory_slider(ui: &mut Ui, mb: u64, min: u64, max: u64, mut set: impl FnMut(u64)) -> bool {
    let mut value = mb.clamp(min, max);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 70.0).max(120.0);
        changed = ui.add(egui::Slider::new(&mut value, min..=max).step_by(memory::STEP_MB as f64).show_value(false)).changed();
        ui.label(RichText::new(fmt_mb(value)).family(theme::semibold()).color(text()));
    });
    if changed || value != mb {
        set(value);
    }
    changed
}

/// A label and value row in the profile.
fn detail(ui: &mut Ui, label: &str, value: &str, copy: Option<&str>) {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(90.0, 20.0), egui::Label::new(RichText::new(label).color(text_faint())));
        ui.label(RichText::new(value).color(text()));
        if let Some(copy) = copy
            && ui.link("Copy").clicked()
        {
            ui.ctx().copy_text(copy.to_owned());
        }
    });
}

fn dashed_uuid(u: &str) -> String {
    if u.len() == 32 && !u.contains('-') {
        format!("{}-{}-{}-{}-{}", &u[..8], &u[8..12], &u[12..16], &u[16..20], &u[20..])
    } else {
        u.to_owned()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn uuid_dashes() {
        assert_eq!(super::dashed_uuid("0123456789abcdef0123456789abcdef"), "01234567-89ab-cdef-0123-456789abcdef");
        assert_eq!(super::dashed_uuid("already-dashed"), "already-dashed");
    }
}

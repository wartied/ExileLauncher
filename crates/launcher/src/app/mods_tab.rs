//! The Mods tab and the "Add mods" dialog.

use eframe::egui::{Align, Layout, Rect, Sense, UiBuilder, pos2};

use super::*;
use crate::loader::{self, LoaderKind};

const ROW_GAP: f32 = 1.0;

impl ExileApp {
    /// Re-reads an instance's mods folder and Exile's record of it.
    pub(super) fn refresh_mods(&mut self, id: &str) {
        let Some(inst) = self.instances.iter().find(|i| i.id == id).cloned() else { return };
        let list = ModList::load(&inst);
        let views = mods::scan(&inst, &list);
        self.mods.insert(id.to_owned(), ModsCache { list, views });
    }

    fn mod_target(&self, inst: &Instance) -> Option<mods::Target> {
        Some(mods::Target {
            game: inst.version.clone(),
            loader: inst.loader.as_ref()?.kind,
            curseforge_key: self.settings.curseforge_key(),
        })
    }

    fn push_notice(&mut self, id: &str, color: Color32, text: impl Into<String>) {
        self.mod_notices.entry(id.to_owned()).or_default().push((color, text.into()));
    }

    fn gpu_is_weak(&self) -> bool {
        self.gpu.as_deref().is_some_and(mods::weak_gpu)
    }

    // -- Actions --------------------------------------------------------------

    fn add_fabric(&mut self, id: &str) {
        let Some(inst) = self.instances.iter().find(|i| i.id == id).cloned() else { return };
        self.loader_jobs.insert(id.to_owned());
        let id = id.to_owned();
        self.spawn(move || Event::LoaderReady {
            instance_id: id,
            result: loader::latest(LoaderKind::Fabric, &inst.version).map_err(|e| format!("{e:#}")),
        });
    }

    /// Turns Exile Client on for an instance: Fabric first if needed, then
    /// the companion mods it is built to run with.
    pub(super) fn setup_client(&mut self, id: &str) {
        let Some(inst) = self.instances.iter().find(|i| i.id == id).cloned() else { return };
        if inst.loader.is_none() {
            self.client_setup.insert(id.to_owned());
            if !self.loader_jobs.contains(id) {
                self.add_fabric(id);
            }
        } else {
            self.install_companions(id);
        }
    }

    fn install_companions(&mut self, id: &str) {
        let installed: HashSet<String> = self
            .mods
            .get(id)
            .map(|c| c.list.mods.iter().map(|m| m.slug.clone()).collect())
            .unwrap_or_default();
        for (slug, title) in crate::client::COMPANIONS {
            if installed.contains(*slug) {
                continue;
            }
            let hit = SearchHit {
                source: Source::Modrinth,
                project_id: (*slug).to_owned(),
                slug: (*slug).to_owned(),
                title: (*title).to_owned(),
                author: String::new(),
                description: String::new(),
                downloads: 0,
            };
            self.start_mod_install(id, hit);
        }
    }

    fn open_add_mods(&mut self, id: &str) {
        self.dialog = Some(Dialog::AddMods(Box::new(ModBrowser {
            instance_id: id.to_owned(),
            query: String::new(),
            source: Source::Modrinth,
            // Show popular mods straight away.
            edited: Instant::now() - Duration::from_secs(1),
            sent: None,
            results: SearchState::Loading,
        })));
    }

    fn start_search(&mut self, b: &mut ModBrowser) {
        let Some(inst) = self.instances.iter().find(|i| i.id == b.instance_id).cloned() else { return };
        let Some(target) = self.mod_target(&inst) else { return };
        let query = b.query.trim().to_owned();
        let source = b.source;
        self.search_seq += 1;
        let seq = self.search_seq;
        b.sent = Some((query.clone(), source));
        b.results = SearchState::Loading;
        self.spawn(move || Event::ModSearch {
            seq,
            result: mods::search(source, &query, &target).map_err(|e| format!("{e:#}")),
        });
    }

    fn start_mod_install(&mut self, instance_id: &str, hit: SearchHit) {
        let Some(inst) = self.instances.iter().find(|i| i.id == instance_id).cloned() else { return };
        let Some(target) = self.mod_target(&inst) else { return };
        let key = (instance_id.to_owned(), hit.project_id.clone());
        if self.mod_jobs.contains_key(&key) {
            return;
        }
        let progress = Arc::new(Progress::default());
        self.mod_jobs.insert(key, ModJob { title: hit.title.clone(), progress: progress.clone() });
        let installed = self.mods.get(instance_id).map(|c| c.list.clone()).unwrap_or_else(|| ModList::load(&inst));
        let instance_id = instance_id.to_owned();
        self.spawn(move || {
            let result = mods::install(&hit, &target, &installed, &inst.mods_dir(), &progress);
            Event::ModInstalled { instance_id, project_id: hit.project_id, result: result.map_err(|e| format!("{e:#}")) }
        });
    }

    fn toggle_mod(&mut self, inst: &Instance, file: &str, enabled: bool) {
        if let Err(e) = mods::set_enabled(inst, file, enabled) {
            self.push_notice(&inst.id, DANGER, format!("Couldn't change {file}: {e:#}"));
        }
        self.refresh_mods(&inst.id);
    }

    fn remove_mod(&mut self, inst: &Instance, file: &str) {
        let mut list = self.mods.get(&inst.id).map(|c| c.list.clone()).unwrap_or_else(|| ModList::load(inst));
        match mods::remove(inst, &mut list, file).and_then(|extra| list.save(inst).map(|_| extra)) {
            Ok(extra) if !extra.is_empty() => {
                self.push_notice(&inst.id, ACCENT, format!("Also removed {}, no longer needed.", extra.join(", ")))
            }
            Ok(_) => {}
            Err(e) => self.push_notice(&inst.id, DANGER, format!("Couldn't remove {file}: {e:#}")),
        }
        self.refresh_mods(&inst.id);
    }

    pub(super) fn handle_mod_event(&mut self, event: Event) {
        match event {
            Event::ModSearch { seq, result } => {
                if seq != self.search_seq {
                    return; // an older search answered late
                }
                if let Some(Dialog::AddMods(b)) = &mut self.dialog {
                    b.results = match result {
                        Ok(hits) => SearchState::Ready(hits),
                        Err(e) => SearchState::Failed(e),
                    };
                }
            }
            Event::ModInstalled { instance_id, project_id, result } => {
                let title = self
                    .mod_jobs
                    .remove(&(instance_id.clone(), project_id))
                    .map(|j| j.title)
                    .unwrap_or_default();
                let Some(inst) = self.instances.iter().find(|i| i.id == instance_id).cloned() else { return };
                match result {
                    Ok(outcome) => {
                        let mut list = ModList::load(&inst);
                        list.merge(&inst.mods_dir(), outcome.installed);
                        if let Err(e) = list.save(&inst) {
                            self.push_notice(&instance_id, DANGER, format!("Couldn't save the mod list: {e:#}"));
                        }
                        for w in outcome.warnings {
                            self.push_notice(&instance_id, WARN, w);
                        }
                    }
                    Err(e) => self.push_notice(&instance_id, DANGER, format!("Couldn't install {title}: {e}")),
                }
                self.refresh_mods(&instance_id);
            }
            Event::LoaderReady { instance_id, result } => {
                self.loader_jobs.remove(&instance_id);
                match result {
                    Ok(spec) => {
                        if let Some(inst) = self.instance_mut(&instance_id) {
                            inst.loader = Some(spec);
                            let _ = inst.save();
                        }
                        // Download the loader's libraries now, not at first launch.
                        self.start_task(&instance_id, None);
                        self.refresh_mods(&instance_id);
                        if self.client_setup.remove(&instance_id) {
                            self.install_companions(&instance_id);
                        }
                    }
                    Err(e) => self.push_notice(&instance_id, DANGER, format!("Couldn't set up Fabric: {e}")),
                }
            }
            _ => {}
        }
    }

    // -- Mods tab -------------------------------------------------------------

    pub(super) fn mods_tab(&mut self, ui: &mut Ui, inst: &Instance, running: bool) {
        let id = inst.id.clone();
        let busy = running || self.tasks.contains_key(&id);

        if !LoaderKind::Fabric.supports(&inst.release_time) {
            widgets::card(ui, |ui| {
                ui.label(RichText::new("Mods aren't available for this version").family(theme::semibold()).color(TEXT));
                ui.label(
                    RichText::new("Fabric, the mod loader Exile uses, supports Minecraft 1.14 and newer.").color(TEXT_DIM),
                );
            });
            return;
        }

        let Some(spec) = inst.loader.clone() else {
            let setting_up = self.loader_jobs.contains(&id);
            let mut install = false;
            widgets::card(ui, |ui| {
                ui.label(RichText::new("Add mods to this instance").family(theme::semibold()).color(TEXT));
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "Mods run on Fabric, a lightweight mod loader. Exile installs it for you. \
                             Your worlds and settings stay as they are.",
                        )
                        .color(TEXT_DIM),
                    )
                    .wrap(),
                );
                ui.add_space(12.0);
                if setting_up {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(14.0).color(TEXT_DIM));
                        ui.label(RichText::new("Setting up Fabric…").color(TEXT_DIM));
                    });
                } else {
                    install = widgets::filled_button(ui, "Install Fabric", vec2(150.0, 32.0), Kind::Primary, !busy).clicked();
                    if busy {
                        ui.label(RichText::new("Close the game first.").size(12.0).color(TEXT_FAINT));
                    }
                }
            });
            if let Some(notices) = self.mod_notices.get(&id).cloned() {
                ui.add_space(12.0);
                self.show_notices(ui, &id, notices);
            }
            if install {
                self.add_fabric(&id);
            }
            return;
        };

        // Header: loader on the left, the one main action on the right.
        let mut open_browser = false;
        let add_w = 120.0;
        ui.allocate_ui_with_layout(vec2(ui.available_width(), 32.0), Layout::left_to_right(Align::Center), |ui| {
            ui.label(RichText::new(spec.label()).color(TEXT_DIM));
            ui.add_space((ui.available_width() - add_w).max(0.0));
            open_browser = widgets::filled_button(ui, "Add mods", vec2(add_w, 32.0), Kind::Primary, true).clicked();
        });
        if open_browser {
            self.open_add_mods(&id);
        }
        ui.add_space(12.0);

        let views = self.mods.get(&id).map(|c| c.views.clone()).unwrap_or_default();
        let enabled = views.iter().filter(|v| v.enabled).count();

        // Memory: the needs grow with the number of mods.
        let limits = self.limits_for(inst);
        if !limits.fits {
            widgets::notice(
                ui,
                DANGER,
                &format!("This computer may not have enough memory for {enabled} mods. Try disabling some."),
                None,
            );
            ui.add_space(8.0);
        } else if enabled > 10 && inst.memory_mb < limits.recommended_mb {
            let rec = limits.recommended_mb;
            let text = format!(
                "With {enabled} mods, {} of memory is recommended. This instance has {}.",
                fmt_mb(rec),
                fmt_mb(memory::sanitize(inst.memory_mb, limits))
            );
            if widgets::notice(ui, WARN, &text, Some(&format!("Use {}", fmt_mb(rec))))
                && let Some(i) = self.instance_mut(&id)
            {
                i.memory_mb = rec;
                let _ = i.save();
            }
            ui.add_space(8.0);
        }

        // Graphics: warn about demanding mods on integrated graphics.
        if self.gpu_is_weak() {
            let mut seen = HashSet::new();
            for v in views.iter().filter(|v| v.enabled) {
                let Some(note) = v.entry.as_ref().and_then(|e| mods::demanding_note(&e.slug)) else { continue };
                if seen.insert(note) {
                    let gpu = self.gpu.clone().unwrap_or_default();
                    widgets::notice(ui, WARN, &format!("{note} Your graphics: {gpu}."), None);
                    ui.add_space(8.0);
                }
            }
        }

        // Missing dependencies (e.g. removed by hand): offer a one-click fix.
        let present: HashSet<&str> =
            views.iter().filter_map(|v| v.entry.as_ref().map(|e| e.project_id.as_str())).collect();
        let mut missing: Vec<(String, Source, mods::ModRef)> = Vec::new();
        for e in views.iter().filter(|v| v.enabled).filter_map(|v| v.entry.as_ref()) {
            for dep in e.requires.iter().filter(|d| !present.contains(d.id.as_str())) {
                if !missing.iter().any(|(_, _, d)| d.id == dep.id) {
                    missing.push((e.title.clone(), e.source, dep.clone()));
                }
            }
        }
        for (needer, source, dep) in missing {
            if self.mod_jobs.contains_key(&(id.clone(), dep.id.clone())) {
                continue;
            }
            if widgets::notice(ui, WARN, &format!("{needer} needs {}.", dep.title), Some("Install")) {
                let hit = SearchHit {
                    source,
                    project_id: dep.id.clone(),
                    slug: String::new(),
                    title: dep.title.clone(),
                    author: String::new(),
                    description: String::new(),
                    downloads: 0,
                };
                self.start_mod_install(&id, hit);
            }
            ui.add_space(8.0);
        }

        if let Some(notices) = self.mod_notices.get(&id).cloned() {
            self.show_notices(ui, &id, notices);
        }

        // Installs in progress.
        let jobs: Vec<(String, f32)> = self
            .mod_jobs
            .iter()
            .filter(|((iid, _), _)| *iid == id)
            .map(|(_, j)| (j.title.clone(), j.progress.fraction()))
            .collect();
        for (title, f) in &jobs {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(12.0).color(TEXT_DIM));
                ui.label(RichText::new(format!("Installing {title}…")).color(TEXT_DIM));
                if *f > 0.0 {
                    ui.label(RichText::new(format!("{:.0}%", f * 100.0)).color(TEXT_FAINT));
                }
            });
        }
        if !jobs.is_empty() {
            ui.add_space(8.0);
        }

        if views.is_empty() {
            if jobs.is_empty() {
                widgets::card(ui, |ui| {
                    ui.label(RichText::new("No mods yet").family(theme::semibold()).color(TEXT));
                    ui.label(RichText::new("Use Add mods to search Modrinth and install mods with one click.").color(TEXT_DIM));
                });
            }
            return;
        }

        let mut toggled = None;
        let mut removed = None;
        Frame::new()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(4))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = ROW_GAP;
                for (i, v) in views.iter().enumerate() {
                    if i > 0 {
                        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                        ui.painter().hline(r.x_range().shrink(12.0), r.center().y, Stroke::new(1.0, BORDER));
                    }
                    match mod_row(ui, v, !busy) {
                        RowAction::Toggle(on) => toggled = Some((v.file.clone(), on)),
                        RowAction::Remove => removed = Some(v.file.clone()),
                        RowAction::None => {}
                    }
                }
            });
        if busy {
            ui.add_space(6.0);
            ui.label(RichText::new("Close the game to change mods.").size(12.0).color(TEXT_FAINT));
        }
        if let Some((file, on)) = toggled {
            self.toggle_mod(inst, &file, on);
        }
        if let Some(file) = removed {
            self.remove_mod(inst, &file);
        }
    }

    fn show_notices(&mut self, ui: &mut Ui, id: &str, notices: Vec<(Color32, String)>) {
        let mut dismiss = None;
        for (i, (color, text)) in notices.iter().enumerate() {
            if widgets::notice(ui, *color, text, Some("Dismiss")) {
                dismiss = Some(i);
            }
            ui.add_space(8.0);
        }
        if let Some(i) = dismiss
            && let Some(list) = self.mod_notices.get_mut(id)
        {
            list.remove(i);
        }
    }

    // -- Add mods dialog ------------------------------------------------------

    /// Returns whether the dialog stays open.
    pub(super) fn add_mods_dialog(&mut self, modal: egui::Modal, b: &mut ModBrowser) -> bool {
        let Some(inst) = self.instances.iter().find(|i| i.id == b.instance_id).cloned() else { return false };
        let Some(spec) = inst.loader.clone() else { return false };
        let cf_ready = self.settings.curseforge_key().is_some();

        // Search once typing pauses for a moment.
        let want = (b.query.trim().to_owned(), b.source);
        if b.sent.as_ref() != Some(&want) {
            let wait = Duration::from_millis(300).saturating_sub(b.edited.elapsed());
            if wait.is_zero() {
                self.start_search(b);
            } else {
                self.ctx.request_repaint_after(wait);
            }
        }

        let installed: HashSet<(Source, String)> = self
            .mods
            .get(&inst.id)
            .map(|c| c.list.mods.iter().map(|m| (m.source, m.project_id.clone())).collect())
            .unwrap_or_default();
        let weak_gpu = self.gpu_is_weak();
        let mut install = None;
        let mut done = false;
        let ctx = self.ctx.clone();

        let response = modal.show(&ctx, |ui| {
            ui.set_width(640.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 30.0), Layout::left_to_right(Align::Center), |ui| {
                ui.label(RichText::new("Add mods").size(18.0).family(theme::semibold()).color(TEXT));
                let options = [("Modrinth", true), ("CurseForge", cf_ready)];
                let seg_w = ["Modrinth", "CurseForge"]
                    .iter()
                    .map(|l| widgets::text_width(ui, l, &egui::FontId::proportional(13.0)) + 24.0)
                    .sum::<f32>()
                    + 4.0;
                ui.add_space((ui.available_width() - seg_w).max(0.0));
                let active = if b.source == Source::CurseForge { 1 } else { 0 };
                let hint = "CurseForge needs an API key. Add curseforge_api_key to settings.toml.";
                if let Some(i) = widgets::segmented(ui, &options, active, hint) {
                    b.source = if i == 1 { Source::CurseForge } else { Source::Modrinth };
                    b.edited = Instant::now() - Duration::from_secs(1);
                }
            });
            ui.label(
                RichText::new(format!("For {}  ·  Minecraft {}  ·  {}", inst.name, inst.version, spec.kind.name()))
                    .size(12.0)
                    .color(TEXT_DIM),
            );
            ui.add_space(10.0);

            let edit = ui.add(
                egui::TextEdit::singleline(&mut b.query)
                    .hint_text(format!("Search {}", b.source.name()))
                    .desired_width(f32::INFINITY)
                    .margin(vec2(10.0, 7.0)),
            );
            if edit.changed() {
                b.edited = Instant::now();
            }
            ui.add_space(8.0);

            Frame::new()
                .fill(theme::BG)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(CornerRadius::same(RADIUS))
                .inner_margin(Margin::same(4))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(400.0);
                    match &b.results {
                        SearchState::Loading => {
                            ui.centered_and_justified(|ui| {
                                ui.add(egui::Spinner::new().size(18.0).color(TEXT_DIM));
                            });
                        }
                        SearchState::Failed(err) => {
                            ui.vertical_centered(|ui| {
                                ui.add_space(150.0);
                                ui.label(RichText::new("Search failed").color(TEXT));
                                ui.label(RichText::new(err.as_str()).size(12.0).color(TEXT_FAINT));
                            });
                        }
                        SearchState::Ready(hits) if hits.is_empty() => {
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "No mods found for Minecraft {} with {}",
                                        inst.version,
                                        spec.kind.name()
                                    ))
                                    .color(TEXT_FAINT),
                                );
                            });
                        }
                        SearchState::Ready(hits) => {
                            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                for hit in hits {
                                    let state = if self.mod_jobs.contains_key(&(inst.id.clone(), hit.project_id.clone())) {
                                        HitState::Installing
                                    } else if installed.contains(&(hit.source, hit.project_id.clone())) {
                                        HitState::Installed
                                    } else {
                                        HitState::Available
                                    };
                                    let heavy = weak_gpu && mods::demanding_note(&hit.slug).is_some();
                                    if hit_row(ui, hit, state, heavy) {
                                        install = Some(hit.clone());
                                    }
                                }
                            });
                        }
                    }
                });

            widgets::dialog_buttons(ui, |ui| {
                done = widgets::filled_button(ui, "Done", vec2(90.0, 32.0), Kind::Primary, true).clicked();
            });
        });

        if let Some(hit) = install {
            let id = inst.id.clone();
            self.start_mod_install(&id, hit);
        }
        !(done || response.should_close())
    }
}

enum RowAction {
    None,
    Toggle(bool),
    Remove,
}

/// One installed mod: name and details on the left, switch and Remove on the right.
fn mod_row(ui: &mut Ui, v: &ModView, editable: bool) -> RowAction {
    let has_issues = !v.issues.is_empty();
    let height = if has_issues { 70.0 } else { 52.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let mut action = RowAction::None;

    // Controls first, so the text knows how much room is left.
    let controls_w = 150.0;
    let controls = Rect::from_min_max(pos2(rect.right() - controls_w, rect.top()), pos2(rect.right() - 8.0, rect.top() + 52.0));
    // Detached child: the row already reserved its space, and a scoped child
    // would move the layout cursor back up to the controls' bottom edge.
    let mut c = ui.new_child(UiBuilder::new().max_rect(controls).layout(Layout::right_to_left(Align::Center)));
    c.spacing_mut().item_spacing.x = 12.0;
    if widgets::button_enabled(&mut c, editable, "Remove").clicked() {
        action = RowAction::Remove;
    }
    let mut on = v.enabled;
    let r = widgets::toggle(&mut c, &mut on, editable);
    let r = r.on_hover_text(if v.enabled { "Enabled" } else { "Disabled" });
    if r.changed() {
        action = RowAction::Toggle(on);
    }

    let left = rect.left() + 12.0;
    let max_w = (controls.left() - left - 12.0).max(40.0);
    let title_font = egui::FontId::new(14.0, theme::semibold());
    let title_color = if v.enabled { TEXT } else { TEXT_FAINT };
    let title = widgets::truncate(ui, v.title(), title_font.clone(), max_w);
    widgets::paint_text(ui, left, Align::Min, rect.top() + 17.0, &title, title_font, title_color);

    let detail = match &v.entry {
        Some(e) if !v.enabled => format!("{}  ·  Disabled", e.version),
        Some(e) if !e.explicit => format!("{}  ·  Installed as a dependency", e.version),
        Some(e) => format!("{}  ·  {}", e.version, e.source.name()),
        None if v.enabled => format!("{}  ·  Added manually", v.file),
        None => format!("{}  ·  Disabled", v.file),
    };
    let small = egui::FontId::proportional(12.0);
    let detail = widgets::truncate(ui, &detail, small.clone(), max_w);
    widgets::paint_text(ui, left, Align::Min, rect.top() + 36.0, &detail, small.clone(), TEXT_FAINT);

    if has_issues {
        let issues = widgets::truncate(ui, &v.issues.join("  ·  "), small.clone(), max_w);
        widgets::paint_text(ui, left, Align::Min, rect.top() + 55.0, &issues, small, DANGER);
    }
    action
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HitState {
    Available,
    Installing,
    Installed,
}

/// One search result. Returns true when Install was clicked.
fn hit_row(ui: &mut Ui, hit: &SearchHit, state: HitState, heavy: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 74.0), Sense::hover());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), SURFACE_HOVER);
    }
    let mut clicked = false;
    let controls_w = 116.0;
    let controls = Rect::from_min_max(pos2(rect.right() - controls_w, rect.top()), pos2(rect.right() - 10.0, rect.bottom()));
    let mut c = ui.new_child(UiBuilder::new().max_rect(controls).layout(Layout::right_to_left(Align::Center)));
    match state {
        HitState::Available => {
            clicked = widgets::outline_button(&mut c, "Install", vec2(84.0, 30.0), true, ACCENT_LIGHT).clicked();
        }
        HitState::Installing => {
            c.label(RichText::new("Installing…").size(13.0).color(TEXT_DIM));
        }
        HitState::Installed => {
            c.label(RichText::new("Installed").size(13.0).color(ACCENT));
        }
    }

    let left = rect.left() + 12.0;
    let max_w = (controls.left() - left - 8.0).max(40.0);
    let title_font = egui::FontId::new(14.0, theme::semibold());
    let title = widgets::truncate(ui, &hit.title, title_font.clone(), max_w * 0.7);
    let title_w = widgets::text_width(ui, &title, &title_font);
    widgets::paint_text(ui, left, Align::Min, rect.top() + 17.0, &title, title_font, TEXT);
    let small = egui::FontId::proportional(12.0);
    if !hit.author.is_empty() {
        let by = widgets::truncate(ui, &format!("by {}", hit.author), small.clone(), (max_w - title_w - 8.0).max(0.0));
        widgets::paint_text(ui, left + title_w + 8.0, Align::Min, rect.top() + 17.0, &by, small.clone(), TEXT_FAINT);
    }
    let desc = widgets::truncate(ui, &hit.description, small.clone(), max_w);
    widgets::paint_text(ui, left, Align::Min, rect.top() + 37.0, &desc, small.clone(), TEXT_DIM);
    let mut meta = format!("{} downloads", widgets::short_count(hit.downloads));
    if heavy {
        meta.push_str("  ·  ");
    }
    widgets::paint_text(ui, left, Align::Min, rect.top() + 56.0, &meta, small.clone(), TEXT_FAINT);
    if heavy {
        let x = left + widgets::text_width(ui, &meta, &small);
        widgets::paint_text(ui, x, Align::Min, rect.top() + 56.0, "Needs a strong graphics card", small, WARN);
    }
    clicked
}

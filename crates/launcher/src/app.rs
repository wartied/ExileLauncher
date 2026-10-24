//! The launcher window: state, background work and every screen.
//!
//! All slow work (network, disk, process scanning) runs on worker threads
//! that report back through a channel and wake the UI with `request_repaint`.
//! The UI thread only ever reads atomics or owned state, so it never stalls.

mod mods_tab;

use std::collections::{HashMap, HashSet};
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, ColorImage, CornerRadius, Frame, Id, Key, Layout, Margin, RichText, ScrollArea, Stroke,
    TextureHandle, TextureOptions, Ui, vec2,
};

use crate::auth::{self, Session};
use crate::instance::{self, Instance, Settings, unix_now};
use crate::launch::{self, ExistingGame, Exited, Games, LaunchAuth};
use crate::loader::LoaderSpec;
use crate::memory::{self, LaunchCheck, Limits, SystemMemory, fmt_mb};
use crate::mods::{self, InstallOutcome, ModList, ModView, SearchHit, Source};
use crate::net::{self, Progress};
use crate::theme::{self, *};
use crate::versions::{self, ManifestEntry};
use crate::widgets::{self, Kind};
use crate::{install, paths, skin};

// ---------------------------------------------------------------------------
// State

enum Account {
    SignedOut,
    SigningIn { cancel: Arc<AtomicBool> },
    SignedIn { session: Session, refreshing: bool },
}

enum Versions {
    Loading,
    Ready(Vec<ManifestEntry>),
    Failed(String),
}

struct Task {
    progress: Arc<Progress>,
    /// Heap of the game this task will start, if it is a launch.
    launch_heap: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Mods,
}

/// Installed mods of one instance, as last read from disk.
struct ModsCache {
    list: ModList,
    views: Vec<ModView>,
}

impl ModsCache {
    fn enabled_count(&self) -> usize {
        self.views.iter().filter(|v| v.enabled).count()
    }
}

enum SearchState {
    Loading,
    Ready(Vec<SearchHit>),
    Failed(String),
}

/// State of the "Add mods" dialog.
struct ModBrowser {
    instance_id: String,
    query: String,
    source: Source,
    /// Last time the query changed; searches wait for typing to pause.
    edited: Instant,
    /// Query and source of the request in flight or last answered.
    sent: Option<(String, Source)>,
    results: SearchState,
}

/// A mod being installed into an instance.
struct ModJob {
    title: String,
    progress: Arc<Progress>,
}

enum Dialog {
    NewInstance { name: String, search: String, version: Option<String>, client: bool },
    Rename { id: String, name: String },
    Delete { id: String },
    ConfirmLaunch { id: String, message: String },
    ConfirmStop { id: String },
    Message { title: String, body: String },
    AddMods(Box<ModBrowser>),
}

enum TaskOutcome {
    Installed,
    Launched { child: Child, heap_mb: u64 },
    Cancelled,
    Failed(String),
}

enum Event {
    Versions(Result<Vec<ManifestEntry>, String>),
    SignIn(Result<Session, String>),
    Refreshed(Result<Session, (String, bool)>),
    Face(Option<skin::Face>),
    Existing(Vec<ExistingGame>),
    TaskDone { id: String, outcome: TaskOutcome, session: Option<Session>, sign_out: bool },
    ModSearch { seq: u64, result: Result<Vec<SearchHit>, String> },
    ModInstalled { instance_id: String, project_id: String, result: Result<InstallOutcome, String> },
    LoaderReady { instance_id: String, result: Result<LoaderSpec, String> },
}

pub struct ExileApp {
    tx: Sender<Event>,
    rx: Receiver<Event>,
    ctx: egui::Context,
    settings: Settings,
    instances: Vec<Instance>,
    selected: Option<String>,
    versions: Versions,
    account: Account,
    face: Option<TextureHandle>,
    tasks: HashMap<String, Task>,
    errors: HashMap<String, String>,
    last_exit: HashMap<String, Exited>,
    games: Games,
    sys: SystemMemory,
    last_probe: Instant,
    last_poll: Instant,
    dialog: Option<Dialog>,
    tab: Tab,
    mods: HashMap<String, ModsCache>,
    /// Keyed by (instance id, project id).
    mod_jobs: HashMap<(String, String), ModJob>,
    /// Per instance: messages from recent mod operations, with their colour
    /// (accent = info, amber = warning, red = error). Dismissed by the user.
    mod_notices: HashMap<String, Vec<(Color32, String)>>,
    /// Instances currently fetching a mod loader version.
    loader_jobs: HashSet<String>,
    /// Sequence number of the newest mod search; older answers are dropped.
    search_seq: u64,
    /// OpenGL renderer string, read once from the launcher's own context.
    gpu: Option<String>,
    /// Instances waiting for Fabric before Exile Client's companion mods install.
    client_setup: HashSet<String>,
}

impl ExileApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let (tx, rx) = channel();
        let settings = Settings::load();
        let instances = instance::load_all();
        let selected = settings
            .selected
            .clone()
            .filter(|s| instances.iter().any(|i| &i.id == s))
            .or_else(|| instances.first().map(|i| i.id.clone()));

        let mut app = Self {
            tx,
            rx,
            ctx: cc.egui_ctx.clone(),
            settings,
            instances,
            selected,
            versions: Versions::Loading,
            account: Account::SignedOut,
            face: None,
            tasks: HashMap::new(),
            errors: HashMap::new(),
            last_exit: HashMap::new(),
            games: Games::default(),
            sys: memory::probe(),
            last_probe: Instant::now(),
            last_poll: Instant::now(),
            dialog: None,
            tab: Tab::Overview,
            mods: HashMap::new(),
            mod_jobs: HashMap::new(),
            mod_notices: HashMap::new(),
            loader_jobs: HashSet::new(),
            search_seq: 0,
            gpu: None,
            client_setup: HashSet::new(),
        };
        if let Some(id) = app.selected.clone() {
            app.refresh_mods(&id);
        }

        app.load_versions();
        if let Some(session) = auth::load_saved() {
            app.set_face(skin::cached_face());
            let refresh = session.needs_refresh();
            app.fetch_face(session.skin_url.clone());
            app.account = Account::SignedIn { session, refreshing: false };
            if refresh {
                app.refresh_session();
            }
        }
        let instances = app.instances.clone();
        app.spawn(move || Event::Existing(launch::find_existing(&instances)));
        app
    }

    fn spawn(&self, work: impl FnOnce() -> Event + Send + 'static) {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(work());
            ctx.request_repaint();
        });
    }

    fn load_versions(&mut self) {
        self.versions = Versions::Loading;
        self.spawn(|| Event::Versions(versions::load_manifest().map_err(|e| format!("{e:#}"))));
    }

    fn fetch_face(&self, url: Option<String>) {
        self.spawn(move || Event::Face(skin::load_face(url.as_deref()).ok()));
    }

    fn set_face(&mut self, face: Option<skin::Face>) {
        self.face = face.map(|px| {
            self.ctx.load_texture("player-face", ColorImage::from_rgba_unmultiplied([8, 8], &px), TextureOptions::NEAREST)
        });
    }

    fn session(&self) -> Option<&Session> {
        match &self.account {
            Account::SignedIn { session, .. } => Some(session),
            _ => None,
        }
    }

    fn selected_instance(&self) -> Option<&Instance> {
        let id = self.selected.as_ref()?;
        self.instances.iter().find(|i| &i.id == id)
    }

    fn instance_mut(&mut self, id: &str) -> Option<&mut Instance> {
        self.instances.iter_mut().find(|i| i.id == id)
    }

    fn select(&mut self, id: Option<String>) {
        if let Some(id) = &id {
            self.refresh_mods(id);
        }
        self.selected = id;
        self.settings.selected = self.selected.clone();
        self.settings.save();
    }

    fn show_message(&mut self, title: &str, body: impl Into<String>) {
        self.dialog = Some(Dialog::Message { title: title.into(), body: body.into() });
    }

    fn manifest_entry(&self, version: &str) -> Option<ManifestEntry> {
        match &self.versions {
            Versions::Ready(list) => list.iter().find(|v| v.id == version).cloned(),
            _ => None,
        }
    }

    // -- Account ------------------------------------------------------------

    fn client_id_or_explain(&mut self) -> Option<String> {
        let id = self.settings.client_id();
        if id.is_none() {
            self.show_message(
                "Microsoft sign-in isn't set up",
                format!(
                    "This build of Exile has no Microsoft application ID. Add one to\n{}\nas  microsoft_client_id = \"…\"  and restart the launcher.",
                    paths::settings_file().display()
                ),
            );
        }
        id
    }

    fn sign_in(&mut self) {
        let Some(client_id) = self.client_id_or_explain() else { return };
        let cancel = Arc::new(AtomicBool::new(false));
        self.account = Account::SigningIn { cancel: cancel.clone() };
        self.spawn(move || {
            let result = auth::sign_in(&client_id, &cancel);
            Event::SignIn(match result {
                Ok(s) => Ok(s),
                Err(e) if net::is_cancelled(&e) => Err(String::new()),
                Err(e) => Err(format!("{e:#}")),
            })
        });
    }

    fn sign_out(&mut self) {
        if let Account::SigningIn { cancel } = &self.account {
            cancel.store(true, Ordering::Relaxed);
        }
        auth::forget();
        skin::clear_cache();
        self.account = Account::SignedOut;
        // Dropping the handle frees the GPU texture.
        self.face = None;
    }

    fn refresh_session(&mut self) {
        let Some(client_id) = self.settings.client_id() else { return };
        let Account::SignedIn { session, refreshing } = &mut self.account else { return };
        if *refreshing {
            return;
        }
        *refreshing = true;
        let session = session.clone();
        self.spawn(move || {
            Event::Refreshed(auth::refresh(&session, &client_id).map_err(|e| (format!("{e:#}"), auth::requires_sign_in(&e))))
        });
    }

    // -- Instances ----------------------------------------------------------

    fn create_instance(&mut self, name: String, entry: ManifestEntry, with_client: bool) {
        let needs = memory::needs_for(&entry.release_time);
        let limits = memory::limits(self.sys.total_mb, needs);
        let name = if name.trim().is_empty() { entry.id.clone() } else { name.trim().to_owned() };
        let inst = Instance {
            id: instance::new_id(&name, &self.instances),
            name,
            version: entry.id.clone(),
            release_time: entry.release_time.clone(),
            memory_mb: limits.recommended_mb,
            created: unix_now(),
            last_played: None,
            client: with_client,
            loader: None,
        };
        if let Err(e) = inst.save() {
            self.show_message("Couldn't create instance", format!("{e:#}"));
            return;
        }
        let id = inst.id.clone();
        self.instances.insert(0, inst);
        self.select(Some(id.clone()));
        if with_client {
            // Fetch Fabric first; the single install task (game + loader) and the
            // companion mods start once its version is known (see LoaderReady).
            self.setup_client(&id);
        } else {
            self.start_task(&id, None);
        }
    }

    /// Memory limits for an instance on this machine, including the extra
    /// its enabled mods need.
    fn limits_for(&self, inst: &Instance) -> Limits {
        let mod_count = match inst.loader {
            Some(_) => self.mods.get(&inst.id).map_or(0, ModsCache::enabled_count),
            None => 0,
        };
        memory::limits(self.sys.total_mb, memory::with_mods(memory::needs_for(&inst.release_time), mod_count))
    }

    /// Entry point for the Play button: memory checks first, then launch.
    /// `low_memory_ok` is set once the user accepted the low-memory warning;
    /// the hard budget is still enforced.
    fn play(&mut self, id: &str, low_memory_ok: bool) {
        if self.session().is_none() || self.games.is_running(id) || self.tasks.contains_key(id) {
            return;
        }
        // Mods may have been added to the folder by hand since the last look.
        self.refresh_mods(id);
        let Some(inst) = self.instances.iter().find(|i| i.id == id) else { return };
        self.sys = memory::probe();
        let limits = self.limits_for(inst);
        let heap_mb = memory::sanitize(inst.memory_mb, limits);
        // Games still starting count too, or two quick clicks could overcommit.
        let starting: u64 = self.tasks.values().filter_map(|t| t.launch_heap).map(memory::footprint_mb).sum();
        let committed = self.games.total_footprint_mb() + starting;
        match memory::check_launch(heap_mb, limits, self.sys, committed) {
            LaunchCheck::Ok => self.start_task(id, Some(heap_mb)),
            LaunchCheck::Warn(_) if low_memory_ok => self.start_task(id, Some(heap_mb)),
            LaunchCheck::Warn(message) => self.dialog = Some(Dialog::ConfirmLaunch { id: id.to_owned(), message }),
            LaunchCheck::Blocked(message) => self.show_message("Not enough memory", message),
        }
    }

    /// Installs the instance's version if needed and, with `launch_heap`,
    /// starts the game afterwards.
    fn start_task(&mut self, id: &str, launch_heap: Option<u64>) {
        let Some(inst) = self.instances.iter().find(|i| i.id == id).cloned() else { return };
        let progress = Arc::new(Progress::default());
        self.tasks.insert(id.to_owned(), Task { progress: progress.clone(), launch_heap });
        self.errors.remove(id);
        self.last_exit.remove(id);

        let entry = self.manifest_entry(&inst.version);
        let session = self.session().cloned();
        let client_id = self.settings.client_id();
        let id = id.to_owned();
        self.spawn(move || {
            let mut fresh_session = None;
            let mut sign_out = false;
            let outcome = (|| -> anyhow::Result<TaskOutcome> {
                // Launching requires a valid, license-checked session.
                let auth = match launch_heap {
                    Some(_) => {
                        let mut s = session.ok_or_else(|| anyhow::anyhow!("Sign in to play."))?;
                        if s.needs_refresh() {
                            progress.set_stage("Signing in");
                            let client_id = client_id.ok_or_else(|| anyhow::anyhow!("Microsoft sign-in isn't set up."))?;
                            match auth::refresh(&s, &client_id) {
                                Ok(new) => {
                                    s = new;
                                    fresh_session = Some(s.clone());
                                }
                                Err(e) => {
                                    sign_out = auth::requires_sign_in(&e);
                                    return Err(e);
                                }
                            }
                        }
                        Some(LaunchAuth { name: s.name, uuid: s.uuid, access_token: s.access_token })
                    }
                    None => None,
                };

                progress.set_stage("Checking files");
                if launch_heap.is_none() || !install::is_complete(&inst.version, inst.loader.as_ref()) {
                    let entry = match entry {
                        Some(e) => e,
                        None => versions::load_manifest()?
                            .into_iter()
                            .find(|v| v.id == inst.version)
                            .ok_or_else(|| anyhow::anyhow!("Minecraft {} is no longer available.", inst.version))?,
                    };
                    install::install(&entry, inst.loader.as_ref(), &progress)?;
                }

                match (launch_heap, auth) {
                    (Some(heap_mb), Some(auth)) => {
                        progress.set_stage("Starting Minecraft");
                        // The bundled client is written fresh each launch, so it always matches the launcher.
                        if inst.client {
                            crate::client::install(&inst.mods_dir())?;
                        } else {
                            crate::client::remove(&inst.mods_dir());
                        }
                        let child = launch::spawn(&inst, heap_mb, &auth)?;
                        Ok(TaskOutcome::Launched { child, heap_mb })
                    }
                    _ => Ok(TaskOutcome::Installed),
                }
            })();
            let outcome = match outcome {
                Ok(o) => o,
                Err(e) if net::is_cancelled(&e) => TaskOutcome::Cancelled,
                Err(e) => TaskOutcome::Failed(format!("{e:#}")),
            };
            Event::TaskDone { id, outcome, session: fresh_session, sign_out }
        });
    }

    fn delete_instance(&mut self, id: &str) {
        if self.games.is_running(id) || self.tasks.contains_key(id) {
            return;
        }
        let Some(pos) = self.instances.iter().position(|i| i.id == id) else { return };
        if let Err(e) = self.instances[pos].delete() {
            self.show_message("Couldn't delete instance", format!("{e:#}"));
            return;
        }
        self.instances.remove(pos);
        self.errors.remove(id);
        self.last_exit.remove(id);
        let next = self.instances.get(pos.min(self.instances.len().saturating_sub(1))).map(|i| i.id.clone());
        self.select(next);
    }

    fn open_log(&self, inst: &Instance) {
        let latest = inst.game_dir().join("logs").join("latest.log");
        let path = if latest.is_file() { latest } else { inst.output_log() };
        let _ = open::that(path);
    }

    // -- Event loop ---------------------------------------------------------

    fn handle_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Versions(Ok(list)) => self.versions = Versions::Ready(list),
                Event::Versions(Err(e)) => self.versions = Versions::Failed(e),
                Event::SignIn(result) => match result {
                    Ok(session) => {
                        if let Err(e) = auth::save(&session) {
                            self.show_message("Couldn't save your sign-in", format!("{e:#}"));
                        }
                        self.fetch_face(session.skin_url.clone());
                        self.account = Account::SignedIn { session, refreshing: false };
                        // Bring the launcher back in front of the browser.
                        self.ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    Err(e) => {
                        self.account = Account::SignedOut;
                        if !e.is_empty() {
                            self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                            self.show_message("Sign-in failed", e);
                        }
                    }
                },
                Event::Refreshed(result) => match result {
                    Ok(session) => {
                        let _ = auth::save(&session);
                        self.fetch_face(session.skin_url.clone());
                        self.account = Account::SignedIn { session, refreshing: false };
                    }
                    Err((msg, sign_in_required)) => {
                        if sign_in_required {
                            self.sign_out();
                            self.show_message("Please sign in again", msg);
                        } else if let Account::SignedIn { refreshing, .. } = &mut self.account {
                            // Offline: keep the session; launching will retry.
                            *refreshing = false;
                        }
                    }
                },
                Event::Face(face) => {
                    if face.is_some() && self.session().is_some() {
                        self.set_face(face);
                    }
                }
                Event::Existing(found) => self.games.adopt(found),
                ev @ (Event::ModSearch { .. } | Event::ModInstalled { .. } | Event::LoaderReady { .. }) => {
                    self.handle_mod_event(ev)
                }
                Event::TaskDone { id, outcome, session, sign_out } => {
                    self.tasks.remove(&id);
                    if let Some(session) = session {
                        let _ = auth::save(&session);
                        self.account = Account::SignedIn { session, refreshing: false };
                    }
                    if sign_out {
                        self.sign_out();
                    }
                    match outcome {
                        TaskOutcome::Installed | TaskOutcome::Cancelled => {}
                        TaskOutcome::Launched { child, heap_mb } => {
                            let recording_log = self
                                .instances
                                .iter()
                                .find(|i| i.id == id)
                                .filter(|i| crate::aot::recording(i))
                                .map(Instance::output_log);
                            self.games.add(id.clone(), heap_mb, child, recording_log);
                            if let Some(inst) = self.instance_mut(&id) {
                                inst.last_played = Some(unix_now());
                                let _ = inst.save();
                            }
                        }
                        TaskOutcome::Failed(msg) => {
                            if sign_out {
                                self.show_message("Please sign in again", msg.clone());
                            }
                            self.errors.insert(id, msg);
                        }
                    }
                }
            }
        }
    }

    fn tick(&mut self) {
        if self.last_poll.elapsed() >= Duration::from_secs(1) {
            self.last_poll = Instant::now();
            for exited in self.games.poll() {
                if let Some(inst) = self.instances.iter().find(|i| i.id == exited.instance_id) {
                    self.games.build_aot_cache(inst, exited.heap_mb);
                }
                self.last_exit.insert(exited.instance_id.clone(), exited);
            }
        }
        if self.last_probe.elapsed() >= Duration::from_secs(2) {
            self.last_probe = Instant::now();
            self.sys = memory::probe();
        }
        // Repaint only as often as something on screen can change.
        let next = if !self.tasks.is_empty() || !self.mod_jobs.is_empty() {
            Duration::from_millis(100)
        } else if !self.games.running.is_empty() || matches!(self.account, Account::SigningIn { .. }) {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(2)
        };
        self.ctx.request_repaint_after(next);
    }
}

// ---------------------------------------------------------------------------
// Views

impl eframe::App for ExileApp {
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        if self.gpu.is_none() {
            use eframe::glow::HasContext as _;
            // SAFETY: a plain string query on the live context egui renders with.
            self.gpu = frame.gl().map(|gl| unsafe { gl.get_parameter_string(eframe::glow::RENDERER) });
        }
        self.handle_events();
        self.tick();

        egui::Panel::top("top")
            .exact_size(56.0)
            .show_separator_line(false)
            .frame(
                Frame::new()
                    .fill(SIDEBAR)
                    .inner_margin(Margin::symmetric(20, 0))
                    .stroke(Stroke::NONE),
            )
            .show(ui, |ui| self.top_bar(ui));

        egui::Panel::bottom("status")
            .exact_size(30.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(SIDEBAR).inner_margin(Margin::symmetric(20, 0)))
            .show(ui, |ui| self.status_bar(ui));

        egui::Panel::left("instances")
            .exact_size(260.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::new().fill(SIDEBAR).inner_margin(Margin { left: 12, right: 12, top: 12, bottom: 12 }))
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(32, 26)))
            .show(ui, |ui| self.details(ui));

        // Hairlines between regions, drawn on top so panel fills never cover them.
        let screen = ui.ctx().content_rect();
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Background, Id::unique("exile-lines")));
        let line = Stroke::new(1.0, BORDER);
        painter.hline(screen.x_range(), screen.top() + 56.0, line);
        painter.hline(screen.x_range(), screen.bottom() - 30.0, line);
        painter.vline(screen.left() + 260.0, (screen.top() + 56.0)..=(screen.bottom() - 30.0), line);

        self.dialogs(ui.ctx());
    }
}

impl ExileApp {
    fn top_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new("EXILE").size(15.0).family(theme::semibold()).color(TEXT).extra_letter_spacing(3.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| match &self.account {
                Account::SignedOut => {
                    if widgets::button(ui, "Sign in with Microsoft").clicked() {
                        self.sign_in();
                    }
                }
                Account::SigningIn { cancel } => {
                    let cancel = cancel.clone();
                    if widgets::button(ui, "Cancel").clicked() {
                        cancel.store(true, Ordering::Relaxed);
                        self.account = Account::SignedOut;
                    }
                    ui.label(RichText::new("Finish signing in in your browser…").color(TEXT_DIM));
                    ui.add(egui::Spinner::new().size(14.0).color(TEXT_DIM));
                }
                Account::SignedIn { session, .. } => {
                    let name = session.name.clone();
                    let response = self.account_chip(ui, &name);
                    egui::Popup::menu(&response)
                        .align(egui::RectAlign::BOTTOM_END)
                        .gap(6.0)
                        .width(220.0)
                        .show(|ui| {
                            ui.set_min_width(200.0);
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                widgets::avatar(ui, self.face.as_ref(), &name, 32.0);
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&name).family(theme::semibold()));
                                    ui.label(RichText::new("Minecraft: Java Edition").size(12.0).color(TEXT_DIM));
                                });
                            });
                            ui.add_space(4.0);
                            ui.separator();
                            if ui.add(egui::Button::new("Sign out").frame(false).min_size(vec2(ui.available_width(), 28.0))).clicked() {
                                self.sign_out();
                            }
                        });
                }
            });
        });
    }

    /// Avatar plus username; clicking opens the account menu.
    fn account_chip(&self, ui: &mut Ui, name: &str) -> egui::Response {
        let font = egui::FontId::new(14.0, theme::semibold());
        let text_w = ui.fonts_mut(|f| f.layout_no_wrap(name.to_owned(), font.clone(), TEXT).size().x);
        let size = vec2(10.0 + 26.0 + 10.0 + text_w + 12.0, 36.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let bg = if response.hovered() || response.is_pointer_button_down_on() { SURFACE_HOVER } else { Color32::TRANSPARENT };
        ui.painter().rect_filled(rect, CornerRadius::same(RADIUS), bg);
        let face_rect = egui::Rect::from_min_size(rect.min + vec2(10.0, 5.0), vec2(26.0, 26.0));
        widgets::paint_avatar(ui, face_rect, self.face.as_ref(), name);
        widgets::paint_text(ui, face_rect.right() + 10.0, Align::Min, rect.center().y, name, font, TEXT);
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            let used = self.sys.total_mb.saturating_sub(self.sys.available_mb);
            ui.label(
                RichText::new(format!("Memory  {} used of {}", fmt_mb(used), fmt_mb(self.sys.total_mb)))
                    .size(12.0)
                    .color(TEXT_DIM),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let n = self.games.running.len();
                if n > 0 {
                    ui.label(
                        RichText::new(format!(
                            "{n} running · {} reserved for games",
                            fmt_mb(self.games.total_footprint_mb())
                        ))
                        .size(12.0)
                        .color(TEXT_DIM),
                    );
                    widgets::status_dot(ui, ACCENT);
                }
            });
        });
    }

    fn instance_badge(&self, id: &str) -> Option<(String, Color32)> {
        if let Some(task) = self.tasks.get(id) {
            let f = task.progress.fraction();
            return Some(if task.launch_heap.is_some() && f == 0.0 {
                ("Starting".into(), TEXT_DIM)
            } else {
                (format!("{:.0}%", f * 100.0), TEXT_DIM)
            });
        }
        if let Some(g) = self.games.get(id) {
            return Some(if g.saving_cache { ("Finishing".into(), TEXT_DIM) } else { ("Running".into(), ACCENT) });
        }
        if self.errors.contains_key(id) {
            return Some(("Error".into(), DANGER));
        }
        None
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("sidebar-footer")
            .show_separator_line(false)
            .frame(Frame::new().inner_margin(Margin { left: 0, right: 0, top: 10, bottom: 0 }))
            .show(ui, |ui| {
                let w = ui.available_width();
                if widgets::outline_button(ui, "New instance", vec2(w, 36.0), true, TEXT).clicked() {
                    self.open_new_instance();
                }
            });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            widgets::section_label(ui, "Instances");
        });
        ui.add_space(2.0);

        let mut clicked = None;
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for inst in &self.instances {
                let selected = self.selected.as_deref() == Some(inst.id.as_str());
                let subtitle = match &inst.loader {
                    Some(_) if inst.client => format!("Minecraft {} · Exile", inst.version),
                    Some(l) => format!("Minecraft {} · {}", inst.version, l.kind.name()),
                    None => format!("Minecraft {}", inst.version),
                };
                if widgets::instance_row(ui, selected, &inst.name, &subtitle, self.instance_badge(&inst.id)).clicked() {
                    clicked = Some(inst.id.clone());
                }
            }
            if self.instances.is_empty() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new("Nothing here yet.").color(TEXT_FAINT));
                });
            }
        });
        if let Some(id) = clicked {
            self.select(Some(id));
        }
    }

    fn open_new_instance(&mut self) {
        if matches!(self.versions, Versions::Failed(_)) {
            self.load_versions();
        }
        let latest = match &self.versions {
            Versions::Ready(list) => list.iter().find(|v| v.is_release()).map(|v| v.id.clone()),
            _ => None,
        };
        self.dialog = Some(Dialog::NewInstance { name: String::new(), search: String::new(), version: latest, client: true });
    }

    fn details(&mut self, ui: &mut Ui) {
        let Some(inst) = self.selected_instance().cloned() else {
            self.empty_state(ui);
            return;
        };
        let id = inst.id.clone();
        let running = self.games.get(&id).map(|g| (g.used_mb, g.saving_cache));
        let task = self.tasks.get(&id).map(|t| t.progress.clone());
        let signed_in = self.session().is_some();
        let limits = self.limits_for(&inst);

        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(640.0);

            // Title
            ui.label(RichText::new(&inst.name).size(26.0).family(theme::semibold()).color(TEXT));
            ui.add_space(2.0);
            let mut meta = format!("Minecraft {}", inst.version);
            if let Some(l) = &inst.loader {
                meta.push_str(&format!("  ·  {}", l.label()));
            }
            if let Some(t) = inst.last_played {
                meta.push_str(&format!("  ·  Played {}", widgets::relative_time(t, unix_now())));
            }
            ui.label(RichText::new(meta).color(TEXT_DIM));
            ui.add_space(24.0);

            // Play / progress
            if let Some(p) = &task {
                let stage = p.stage();
                let fraction = p.fraction();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if stage.is_empty() { "Preparing".to_owned() } else { stage }).color(TEXT));
                    if p.total_bytes.load(Ordering::Relaxed) > 0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(format!("{:.0}%", fraction * 100.0)).color(TEXT_DIM));
                        });
                    }
                });
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_height(6.0)
                        .desired_width(ui.available_width())
                        .fill(ACCENT)
                        .corner_radius(CornerRadius::same(3)),
                );
                ui.add_space(4.0);
                if widgets::button(ui, "Cancel").clicked() {
                    p.cancel.store(true, Ordering::Relaxed);
                }
            } else if let Some((used_mb, saving_cache)) = running {
                ui.horizontal(|ui| {
                    widgets::status_dot(ui, if saving_cache { TEXT_DIM } else { ACCENT });
                    let used = if used_mb > 0 { format!(" · using {}", fmt_mb(used_mb)) } else { String::new() };
                    let status = if saving_cache { "Saving startup cache" } else { "Running" };
                    ui.label(RichText::new(format!("{status}{used}")).color(TEXT));
                });
                if saving_cache {
                    ui.add_space(2.0);
                    ui.label(RichText::new("Takes about a minute, once. Later launches start faster.").color(TEXT_DIM));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if widgets::button(ui, "Force close").clicked() {
                        self.dialog = Some(Dialog::ConfirmStop { id: id.clone() });
                    }
                });
            } else {
                let can_play = signed_in && limits.fits;
                let response = widgets::filled_button(ui, "Play", vec2(180.0, 44.0), Kind::Primary, can_play);
                if response.clicked() {
                    self.play(&id, false);
                }
                ui.add_space(6.0);
                if !signed_in {
                    ui.label(RichText::new("Sign in with your Microsoft account to play.").color(TEXT_DIM));
                } else if let Some(err) = self.errors.get(&id) {
                    ui.label(RichText::new(err).color(DANGER));
                } else if let Some(exit) = self.last_exit.get(&id).filter(|e| !e.killed) {
                    match exit.code {
                        Some(code) if code != 0 && exit.uptime_secs < 15 => {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(format!("Minecraft closed right after starting (exit code {code}).")).color(DANGER));
                                if ui.link("View log").clicked() {
                                    self.open_log(&inst);
                                }
                            });
                        }
                        Some(code) if code != 0 => {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(format!("The last session ended with an error (exit code {code}).")).color(WARN));
                                if ui.link("View log").clicked() {
                                    self.open_log(&inst);
                                }
                            });
                        }
                        _ => {}
                    }
                }
            }

            ui.add_space(20.0);
            let mods_label = match self.mods.get(&id).map(|m| m.views.len()) {
                Some(n) if n > 0 && inst.loader.is_some() => format!("Mods  {n}"),
                _ => "Mods".to_owned(),
            };
            let active = if self.tab == Tab::Mods { 1 } else { 0 };
            if let Some(i) = widgets::tabs(ui, &["Overview", &mods_label], active) {
                self.tab = if i == 1 { Tab::Mods } else { Tab::Overview };
                if self.tab == Tab::Mods {
                    self.refresh_mods(&id);
                }
            }
            ui.add_space(16.0);
            match self.tab {
                Tab::Overview => {
                    if crate::client::supports(&inst.version) {
                        self.client_card(ui, &inst, running.is_some() || task.is_some());
                        ui.add_space(20.0);
                    }
                    self.memory_card(ui, &inst, limits, running.is_some());
                    ui.add_space(20.0);
                    self.manage_card(ui, &inst, running.is_some() || task.is_some());
                }
                Tab::Mods => self.mods_tab(ui, &inst, running.is_some()),
            }
        });
    }

    fn memory_card(&mut self, ui: &mut Ui, inst: &Instance, limits: Limits, running: bool) {
        widgets::section_label(ui, "Memory");
        ui.add_space(2.0);
        let id = inst.id.clone();
        let mut value = memory::sanitize(inst.memory_mb, limits);
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Allocated to the game").color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(fmt_mb(value)).family(theme::semibold()).color(TEXT));
                });
            });
            ui.add_space(6.0);
            let enabled = limits.max_mb > limits.min_mb;
            ui.spacing_mut().slider_width = ui.available_width();
            let response = ui.add_enabled(
                enabled,
                egui::Slider::new(&mut value, limits.min_mb..=limits.max_mb)
                    .step_by(memory::STEP_MB as f64)
                    .show_value(false),
            );
            ui.add_space(4.0);
            let hint = if !limits.fits {
                RichText::new(format!(
                    "This computer doesn't have enough memory for this version (needs {} for the game plus room for Windows).",
                    fmt_mb(limits.min_mb)
                ))
                .color(DANGER)
            } else {
                RichText::new(format!(
                    "Recommended {}  ·  {} – {} available on this computer",
                    fmt_mb(limits.recommended_mb),
                    fmt_mb(limits.min_mb),
                    fmt_mb(limits.max_mb)
                ))
                .size(12.0)
                .color(TEXT_DIM)
            };
            ui.label(hint);
            if running {
                ui.label(RichText::new("Changes apply the next time you play.").size(12.0).color(TEXT_FAINT));
            }
            if (response.changed() || response.drag_stopped())
                && let Some(inst) = self.instance_mut(&id)
            {
                inst.memory_mb = value;
                // Write once the user lets go, not on every pixel of a drag.
                if !response.dragged() {
                    let _ = inst.save();
                }
            }
        });
    }

    /// Exile Client on/off for versions the bundled client supports.
    fn client_card(&mut self, ui: &mut Ui, inst: &Instance, busy: bool) {
        widgets::section_label(ui, "Exile Client");
        ui.add_space(2.0);
        let mut on = inst.client;
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::toggle(ui, &mut on, !busy);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let version = crate::client::version().unwrap_or("");
                    ui.label(RichText::new(format!("Exile Client {version}")).color(TEXT));
                    ui.label(
                        RichText::new("Measured performance tweaks, a minimal HUD and Frame Budget. In game, open it from the pause menu or with Right Shift.")
                            .size(12.0)
                            .color(TEXT_DIM),
                    );
                });
            });
        });
        if on != inst.client {
            let id = inst.id.clone();
            if let Some(i) = self.instance_mut(&id) {
                i.client = on;
                let _ = i.save();
            }
            if on {
                self.setup_client(&id);
            }
        }
    }

    fn manage_card(&mut self, ui: &mut Ui, inst: &Instance, busy: bool) {
        widgets::section_label(ui, "Instance");
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            if widgets::button(ui, "Open folder").clicked() {
                let _ = std::fs::create_dir_all(inst.game_dir());
                let _ = open::that(inst.game_dir());
            }
            let has_log = inst.output_log().is_file() || inst.game_dir().join("logs").join("latest.log").is_file();
            if widgets::button_enabled(ui, has_log, "View log").clicked() {
                self.open_log(inst);
            }
            if widgets::button(ui, "Rename").clicked() {
                self.dialog = Some(Dialog::Rename { id: inst.id.clone(), name: inst.name.clone() });
            }
            let w = widgets::text_width(ui, "Delete", &egui::FontId::proportional(14.0)) + 32.0;
            let delete = widgets::outline_button(ui, "Delete", vec2(w, widgets::BUTTON_HEIGHT), !busy, DANGER);
            if delete.clicked() {
                self.dialog = Some(Dialog::Delete { id: inst.id.clone() });
            }
        });
    }

    fn empty_state(&mut self, ui: &mut Ui) {
        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.add_space((ui.available_height() / 2.0 - 90.0).max(0.0));
            ui.label(RichText::new("No instances yet").size(22.0).family(theme::semibold()).color(TEXT));
            ui.add_space(4.0);
            ui.label(RichText::new("Create an instance to pick a Minecraft version and install it.").color(TEXT_DIM));
            ui.add_space(20.0);
            if widgets::filled_button(ui, "Create instance", vec2(180.0, 40.0), Kind::Primary, true).clicked() {
                self.open_new_instance();
            }
        });
    }

    // -- Dialogs ------------------------------------------------------------

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else { return };
        let mut keep: bool;
        let modal = egui::Modal::new(Id::unique("exile-dialog")).frame(widgets::dialog_frame()).backdrop_color(Color32::from_black_alpha(140));

        match &mut dialog {
            Dialog::AddMods(browser) => keep = self.add_mods_dialog(modal, browser),
            Dialog::Message { title, body } => {
                let r = modal.show(ctx, |ui| {
                    ui.set_width(380.0);
                    widgets::dialog_title(ui, title);
                    ui.label(RichText::new(body.as_str()).color(TEXT_DIM));
                    let mut ok = false;
                    widgets::dialog_buttons(ui, |ui| {
                        ok = widgets::filled_button(ui, "OK", vec2(90.0, 32.0), Kind::Primary, true).clicked();
                    });
                    ok || ui.input(|i| i.key_pressed(Key::Enter))
                });
                keep = !(r.inner || r.should_close());
            }
            Dialog::Rename { id, name } => {
                let mut save = false;
                let r = modal.show(ctx, |ui| {
                    ui.set_width(380.0);
                    widgets::dialog_title(ui, "Rename instance");
                    let edit = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY).margin(vec2(10.0, 7.0)));
                    edit.request_focus();
                    let valid = !name.trim().is_empty();
                    let mut cancel = false;
                    widgets::dialog_buttons(ui, |ui| {
                        save = widgets::filled_button(ui, "Save", vec2(90.0, 32.0), Kind::Primary, valid).clicked();
                        cancel = widgets::button(ui, "Cancel").clicked();
                    });
                    save |= valid && ui.input(|i| i.key_pressed(Key::Enter));
                    cancel
                });
                if save {
                    let (id, name) = (id.clone(), name.trim().to_owned());
                    if let Some(inst) = self.instance_mut(&id) {
                        inst.name = name;
                        let _ = inst.save();
                    }
                }
                keep = !(save || r.inner || r.should_close());
            }
            Dialog::Delete { id } => {
                let id = id.clone();
                let name = self.instances.iter().find(|i| i.id == id).map(|i| i.name.clone()).unwrap_or_default();
                let r = modal.show(ctx, |ui| {
                    ui.set_width(400.0);
                    widgets::dialog_title(ui, &format!("Delete \u{201c}{name}\u{201d}?"));
                    ui.label(
                        RichText::new("Its worlds, screenshots, resource packs and settings will be permanently deleted.")
                            .color(TEXT_DIM),
                    );
                    let (mut del, mut cancel) = (false, false);
                    widgets::dialog_buttons(ui, |ui| {
                        del = widgets::filled_button(ui, "Delete", vec2(90.0, 32.0), Kind::Danger, true).clicked();
                        cancel = widgets::button(ui, "Cancel").clicked();
                    });
                    (del, cancel)
                });
                let (del, cancel) = r.inner;
                if del {
                    self.delete_instance(&id);
                }
                keep = !(del || cancel || r.should_close());
            }
            Dialog::ConfirmLaunch { id, message } => {
                let r = modal.show(ctx, |ui| {
                    ui.set_width(400.0);
                    widgets::dialog_title(ui, "Low memory");
                    ui.label(RichText::new(message.as_str()).color(TEXT_DIM));
                    let (mut go, mut cancel) = (false, false);
                    widgets::dialog_buttons(ui, |ui| {
                        go = widgets::filled_button(ui, "Play anyway", vec2(120.0, 32.0), Kind::Primary, true).clicked();
                        cancel = widgets::button(ui, "Cancel").clicked();
                    });
                    (go, cancel)
                });
                let (go, cancel) = r.inner;
                if go {
                    let id = id.clone();
                    self.play(&id, true);
                }
                keep = !(go || cancel || r.should_close());
            }
            Dialog::ConfirmStop { id } => {
                let r = modal.show(ctx, |ui| {
                    ui.set_width(400.0);
                    widgets::dialog_title(ui, "Force close Minecraft?");
                    ui.label(
                        RichText::new("Anything since the last autosave will be lost. Quitting from the game menu is safer.")
                            .color(TEXT_DIM),
                    );
                    let (mut stop, mut cancel) = (false, false);
                    widgets::dialog_buttons(ui, |ui| {
                        stop = widgets::filled_button(ui, "Force close", vec2(120.0, 32.0), Kind::Danger, true).clicked();
                        cancel = widgets::button(ui, "Cancel").clicked();
                    });
                    (stop, cancel)
                });
                let (stop, cancel) = r.inner;
                if stop {
                    let id = id.clone();
                    self.games.kill(&id);
                }
                keep = !(stop || cancel || r.should_close());
            }
            Dialog::NewInstance { name, search, version, client } => {
                let mut create = None;
                let mut retry = false;
                let mut show_snapshots = self.settings.show_snapshots;
                let r = modal.show(ctx, |ui| {
                    ui.set_width(460.0);
                    widgets::dialog_title(ui, "New instance");
                    ui.add_space(8.0);

                    ui.label(RichText::new("Name").size(12.0).color(TEXT_DIM));
                    ui.add(
                        egui::TextEdit::singleline(name)
                            .hint_text(version.clone().unwrap_or_else(|| "My world".into()))
                            .desired_width(f32::INFINITY)
                            .margin(vec2(10.0, 7.0)),
                    );
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Version").size(12.0).color(TEXT_DIM));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.checkbox(&mut show_snapshots, RichText::new("Show snapshots").size(12.0).color(TEXT_DIM));
                        });
                    });
                    ui.add(
                        egui::TextEdit::singleline(search)
                            .hint_text("Search versions")
                            .desired_width(f32::INFINITY)
                            .margin(vec2(10.0, 7.0)),
                    );
                    ui.add_space(6.0);

                    Frame::new()
                        .fill(theme::BG)
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(CornerRadius::same(RADIUS))
                        .inner_margin(Margin::same(4))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.set_height(240.0);
                            match &self.versions {
                                Versions::Loading => {
                                    ui.centered_and_justified(|ui| {
                                        ui.add(egui::Spinner::new().size(18.0).color(TEXT_DIM));
                                    });
                                }
                                Versions::Failed(err) => {
                                    ui.vertical_centered(|ui| {
                                        ui.add_space(70.0);
                                        ui.label(RichText::new("Couldn't load the version list.").color(TEXT));
                                        ui.label(RichText::new(err.as_str()).size(12.0).color(TEXT_FAINT));
                                        ui.add_space(6.0);
                                        retry = widgets::button(ui, "Try again").clicked();
                                    });
                                }
                                Versions::Ready(list) => {
                                    let query = search.trim().to_lowercase();
                                    let mut shown: Vec<&ManifestEntry> = list
                                        .iter()
                                        .filter(|v| show_snapshots || v.is_release())
                                        .filter(|v| query.is_empty() || v.id.to_lowercase().contains(&query))
                                        .collect();
                                    // "1.7" should list 1.7.x before 1.21.7; the sort is stable,
                                    // so each group stays newest-first.
                                    shown.sort_by_key(|v| !v.id.to_lowercase().starts_with(&query));
                                    // The highlighted row is always what Create will use: never
                                    // keep a selection the filter has hidden.
                                    if !shown.iter().any(|v| version.as_deref() == Some(v.id.as_str())) {
                                        *version = shown.first().map(|v| v.id.clone());
                                    }
                                    if shown.is_empty() {
                                        ui.centered_and_justified(|ui| {
                                            ui.label(RichText::new("No matching versions").color(TEXT_FAINT));
                                        });
                                    }
                                    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        for v in shown {
                                            let selected = version.as_deref() == Some(v.id.as_str());
                                            let r = version_row(ui, v, selected);
                                            if r.clicked() {
                                                *version = Some(v.id.clone());
                                            }
                                            if r.double_clicked() {
                                                create = Some(v.clone());
                                            }
                                        }
                                    });
                                }
                            }
                        });

                    ui.add_space(8.0);
                    let entry = match (&self.versions, version.as_ref()) {
                        (Versions::Ready(list), Some(id)) => list.iter().find(|v| &v.id == id).cloned(),
                        _ => None,
                    };
                    if let Some(e) = &entry {
                        let limits = memory::limits(self.sys.total_mb, memory::needs_for(&e.release_time));
                        let text = if limits.fits {
                            format!("Uses {} of memory by default. You can change this later.", fmt_mb(limits.recommended_mb))
                        } else {
                            "This computer doesn't have enough memory to run this version.".to_owned()
                        };
                        ui.label(RichText::new(text).size(12.0).color(if limits.fits { TEXT_FAINT } else { DANGER }));
                        if crate::client::supports(&e.id) {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                widgets::toggle(ui, client, true);
                                ui.add_space(4.0);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.label(RichText::new("Exile Client").color(TEXT));
                                    ui.label(
                                        RichText::new("Performance, HUD and Frame Budget. Also installs Fabric, Sodium and Lithium.")
                                            .size(12.0)
                                            .color(TEXT_FAINT),
                                    );
                                });
                            });
                        } else if let Some(v) = crate::client::minecraft_version() {
                            ui.label(RichText::new(format!("Exile Client is available for Minecraft {v}.")).size(12.0).color(TEXT_FAINT));
                        }
                    }

                    let mut cancel = false;
                    widgets::dialog_buttons(ui, |ui| {
                        if widgets::filled_button(ui, "Create", vec2(100.0, 32.0), Kind::Primary, entry.is_some()).clicked() {
                            create = entry.clone();
                        }
                        cancel = widgets::button(ui, "Cancel").clicked();
                    });
                    if ui.input(|i| i.key_pressed(Key::Enter)) && create.is_none() {
                        create = entry;
                    }
                    cancel
                });
                if show_snapshots != self.settings.show_snapshots {
                    self.settings.show_snapshots = show_snapshots;
                    self.settings.save();
                }
                if retry {
                    self.load_versions();
                }
                keep = !(r.inner || r.should_close());
                if let Some(entry) = create {
                    let with_client = *client && crate::client::supports(&entry.id);
                    self.create_instance(std::mem::take(name), entry, with_client);
                    keep = false;
                }
            }
        }
        if keep && self.dialog.is_none() {
            self.dialog = Some(dialog);
        }
    }
}

fn version_row(ui: &mut Ui, v: &ManifestEntry, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), egui::Sense::click());
    let painter = ui.painter();
    let bg = if selected {
        ACCENT_DIM
    } else if response.hovered() {
        SURFACE_HOVER
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, CornerRadius::same(4), bg);
    widgets::paint_text(ui, rect.left() + 12.0, Align::Min, rect.center().y, &v.id, egui::FontId::proportional(14.0), TEXT);
    let right = if v.is_release() {
        v.release_time.get(..10).unwrap_or("").to_owned()
    } else {
        format!("Snapshot  ·  {}", v.release_time.get(..10).unwrap_or(""))
    };
    let color = if selected { TEXT } else { TEXT_FAINT };
    widgets::paint_text(ui, rect.right() - 12.0, Align::Max, rect.center().y, &right, egui::FontId::proportional(12.0), color);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

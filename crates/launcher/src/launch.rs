//! Building the game command line and keeping track of running games.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use anyhow::{Context as _, Result, bail};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::instance::Instance;
use crate::versions::{self, Argument, VersionJson};
use crate::{install, java, loader, memory, paths};

pub struct LaunchAuth {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
}

const CLASSPATH_SEPARATOR: &str = if cfg!(windows) { ";" } else { ":" };

/// G1 settings used by Mojang's own launcher. Short pauses, no tuning games.
const GC_ARGS: &[&str] = &[
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+UseG1GC",
    "-XX:G1NewSizePercent=20",
    "-XX:G1ReservePercent=20",
    "-XX:MaxGCPauseMillis=50",
    "-XX:G1HeapRegionSize=32M",
];

fn classpath(v: &VersionJson) -> Result<String> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for lib in v.libraries.iter().filter(|l| l.allowed()) {
        let Some(rel) = lib.artifact().and_then(|a| a.path) else { continue };
        let path = paths::safe_join(&paths::libraries_dir(), &rel).with_context(|| format!("bad library path {rel}"))?;
        if seen.insert(path.clone()) {
            entries.push(path);
        }
    }
    entries.push(paths::version_jar(&v.id));
    let parts: Vec<String> = entries.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    Ok(parts.join(CLASSPATH_SEPARATOR))
}

/// Extracts LWJGL 2-era natives into the instance's own folder. Per-instance,
/// so two running instances never fight over the same DLLs.
fn extract_natives(v: &VersionJson, dest: &Path) -> Result<()> {
    if dest.exists() {
        // Best effort: a crashed game may have left a file locked.
        let _ = fs::remove_dir_all(dest);
    }
    fs::create_dir_all(dest)?;
    for lib in v.libraries.iter().filter(|l| l.allowed()) {
        let Some(rel) = lib.native_artifact().and_then(|a| a.path) else { continue };
        let jar = paths::safe_join(&paths::libraries_dir(), &rel).with_context(|| format!("bad natives path {rel}"))?;
        let excludes: Vec<String> = lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default();
        let mut zip = zip::ZipArchive::new(File::open(&jar).with_context(|| format!("opening {}", jar.display()))?)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            // enclosed_name rejects absolute paths and `..` (zip-slip).
            let Some(name) = entry.enclosed_name() else { continue };
            let name_str = name.to_string_lossy().replace('\\', "/");
            if excludes.iter().any(|ex| name_str.starts_with(ex.as_str())) {
                continue;
            }
            let out_path = dest.join(&name);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

fn substitute(arg: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = arg.to_owned();
    // Only touch `${...}` placeholders; values themselves are never re-scanned.
    while let Some(start) = out.find("${") {
        let Some(len) = out[start..].find('}') else { break };
        let key = &out[start + 2..start + len];
        let value = vars.get(key).cloned().unwrap_or_default();
        out.replace_range(start..start + len + 1, &value);
        if value.contains("${") {
            break;
        }
    }
    out
}

fn expand(args: &[Argument], vars: &HashMap<&str, String>) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::Plain(s) => out.push(substitute(s, vars)),
            Argument::Conditional { rules, value } => {
                if versions::rules_allow(rules) {
                    out.extend(value.values().iter().map(|s| substitute(s, vars)));
                }
            }
        }
    }
    out
}

/// Assembles the full argument list (everything after the java executable).
pub fn build_args(inst: &Instance, v: &VersionJson, heap_mb: u64, auth: &LaunchAuth) -> Result<Vec<String>> {
    let index = install::asset_index_for(v)?;
    let assets_root = paths::assets_dir();
    let game_dir = inst.game_dir();
    let game_assets = if index.is_virtual { install::virtual_assets_dir(v) } else { assets_root.clone() };
    let natives = inst.natives_dir();
    let s = |p: &Path| p.to_string_lossy().into_owned();

    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", auth.name.clone()),
        ("auth_uuid", auth.uuid.clone()),
        ("auth_access_token", auth.access_token.clone()),
        ("auth_session", format!("token:{}:{}", auth.access_token, auth.uuid)),
        ("auth_xuid", "0".to_owned()),
        ("clientid", "0".to_owned()),
        ("user_type", "msa".to_owned()),
        ("user_properties", "{}".to_owned()),
        ("version_name", v.id.clone()),
        ("version_type", if v.kind.is_empty() { "release".to_owned() } else { v.kind.clone() }),
        ("game_directory", s(&game_dir)),
        ("assets_root", s(&assets_root)),
        ("game_assets", s(&game_assets)),
        ("assets_index_name", v.asset_index.id.clone()),
        ("natives_directory", s(&natives)),
        ("library_directory", s(&paths::libraries_dir())),
        ("classpath_separator", CLASSPATH_SEPARATOR.to_owned()),
        ("classpath", classpath(v)?),
        ("launcher_name", "Exile".to_owned()),
        ("launcher_version", env!("CARGO_PKG_VERSION").to_owned()),
    ]);

    let mut args = memory::jvm_args(heap_mb);
    args.extend(GC_ARGS.iter().map(|s| s.to_string()));
    // Log4Shell: belt and braces. The flag covers log4j >= 2.10; Mojang's
    // patched logging configs below cover the older log4j in 1.7–1.11.
    args.push("-Dlog4j2.formatMsgNoLookups=true".into());
    if let Some(log) = v.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let path = install::log_config_path(&log.file.id);
        if path.is_file() {
            args.push(log.argument.replace("${path}", &s(&path)));
        }
    }
    match &v.arguments {
        Some(a) if !a.jvm.is_empty() => args.extend(expand(&a.jvm, &vars)),
        _ => {
            if cfg!(windows) {
                args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".into());
            }
            if cfg!(target_os = "macos") {
                args.push("-XstartOnFirstThread".into());
            }
            args.push(format!("-Djava.library.path={}", s(&natives)));
            args.push("-cp".into());
            args.push(vars["classpath"].clone());
        }
    }
    args.push(v.main_class.clone());
    match (&v.arguments, &v.minecraft_arguments) {
        (Some(a), _) if !a.game.is_empty() => args.extend(expand(&a.game, &vars)),
        (_, Some(legacy)) => args.extend(legacy.split_whitespace().map(|a| substitute(a, &vars))),
        _ => bail!("version {} has no game arguments", v.id),
    }
    Ok(args)
}

/// Prepares the instance folder and starts the game.
pub fn spawn(inst: &Instance, heap_mb: u64, auth: &LaunchAuth) -> Result<Child> {
    let v = versions::load_version_json(&inst.version)?;
    let v = match &inst.loader {
        Some(spec) => loader::apply(v, loader::load_profile(spec, &inst.version)?),
        None => v,
    };
    let java = java::java_path(&java::component_for(&v));
    if !java.is_file() {
        bail!("Java runtime is missing. Reinstall the instance.");
    }
    fs::create_dir_all(inst.game_dir())?;
    extract_natives(&v, &inst.natives_dir()).context("extracting native libraries")?;
    let index = install::asset_index_for(&v)?;
    if index.map_to_resources {
        install::copy_assets(&index, &inst.game_dir().join("resources"))?;
    }
    let args = build_args(inst, &v, heap_mb, auth)?;

    let mut log = File::create(inst.output_log())?;
    writeln!(
        log,
        "[Exile] Starting {} ({}) with {} heap\n[Exile] Java: {}\n",
        inst.version,
        inst.name,
        memory::fmt_mb(heap_mb),
        java.display()
    )?;
    let mut cmd = Command::new(&java);
    cmd.args(&args)
        .current_dir(inst.game_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().with_context(|| format!("starting {}", java.display()))
}

// ---------------------------------------------------------------------------
// Running games

pub enum Handle {
    /// Started by this launcher session.
    Child(Child),
    /// Found already running at startup (launcher was restarted).
    Adopted,
}

pub struct RunningGame {
    pub instance_id: String,
    pub pid: u32,
    pub heap_mb: u64,
    pub started: Instant,
    /// Resident memory as last sampled, in MB.
    pub used_mb: u64,
    pub handle: Handle,
    /// Set when the user force-closed it, so the exit isn't reported as a crash.
    pub killed: bool,
}

impl RunningGame {
    pub fn footprint_mb(&self) -> u64 {
        memory::footprint_mb(self.heap_mb)
    }
}

pub struct Exited {
    pub instance_id: String,
    /// `None` when the exit code is unknown (adopted processes).
    pub code: Option<i32>,
    pub uptime_secs: u64,
    pub killed: bool,
}

/// Owns every running game. Child handles are reaped as soon as the game
/// exits, so no zombie processes or OS handles are left behind.
#[derive(Default)]
pub struct Games {
    pub running: Vec<RunningGame>,
    sys: Option<System>,
}

impl Games {
    pub fn is_running(&self, instance_id: &str) -> bool {
        self.running.iter().any(|g| g.instance_id == instance_id)
    }

    pub fn get(&self, instance_id: &str) -> Option<&RunningGame> {
        self.running.iter().find(|g| g.instance_id == instance_id)
    }

    pub fn total_footprint_mb(&self) -> u64 {
        self.running.iter().map(RunningGame::footprint_mb).sum()
    }

    pub fn add(&mut self, instance_id: String, heap_mb: u64, child: Child) {
        self.running.push(RunningGame {
            instance_id,
            pid: child.id(),
            heap_mb,
            started: Instant::now(),
            used_mb: 0,
            handle: Handle::Child(child),
            killed: false,
        });
    }

    fn sys(&mut self) -> &mut System {
        self.sys.get_or_insert_with(System::new)
    }

    /// Reaps exited games and samples memory use of the rest.
    pub fn poll(&mut self) -> Vec<Exited> {
        let pids: Vec<Pid> = self.running.iter().map(|g| Pid::from_u32(g.pid)).collect();
        if pids.is_empty() {
            return Vec::new();
        }
        let sys = self.sys();
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), true, ProcessRefreshKind::nothing().with_memory());
        let alive: HashMap<u32, u64> =
            pids.iter().filter_map(|p| sys.process(*p).map(|proc| (p.as_u32(), proc.memory() / memory::MB))).collect();

        let mut exited = Vec::new();
        self.running.retain_mut(|g| {
            let code = match &mut g.handle {
                Handle::Child(child) => match child.try_wait() {
                    Ok(Some(status)) => Some(status.code()),
                    Ok(None) => None,
                    Err(_) => Some(None),
                },
                Handle::Adopted => (!alive.contains_key(&g.pid)).then_some(None),
            };
            match code {
                Some(code) => {
                    exited.push(Exited {
                        instance_id: g.instance_id.clone(),
                        code,
                        uptime_secs: g.started.elapsed().as_secs(),
                        killed: g.killed,
                    });
                    false
                }
                None => {
                    g.used_mb = alive.get(&g.pid).copied().unwrap_or(g.used_mb);
                    true
                }
            }
        });
        exited
    }

    /// Force-stops a game. The process is reaped by the next `poll`.
    pub fn kill(&mut self, instance_id: &str) {
        let Some(g) = self.running.iter_mut().find(|g| g.instance_id == instance_id) else { return };
        g.killed = true;
        match &mut g.handle {
            Handle::Child(child) => {
                let _ = child.kill();
            }
            Handle::Adopted => {
                let pid = Pid::from_u32(g.pid);
                let sys = self.sys.get_or_insert_with(System::new);
                sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                if let Some(p) = sys.process(pid) {
                    p.kill();
                }
            }
        }
    }

    /// Tracks games found by [`find_existing`].
    pub fn adopt(&mut self, found: Vec<ExistingGame>) {
        for g in found {
            if !self.is_running(&g.instance_id) {
                self.running.push(RunningGame {
                    instance_id: g.instance_id,
                    pid: g.pid,
                    heap_mb: g.heap_mb,
                    started: Instant::now(),
                    used_mb: 0,
                    handle: Handle::Adopted,
                    killed: false,
                });
            }
        }
    }
}

pub struct ExistingGame {
    pub instance_id: String,
    pub pid: u32,
    pub heap_mb: u64,
}

/// Finds games from a previous launcher session that are still running, so
/// their memory keeps counting against the budget. Reads every process's
/// command line, so call it off the UI thread.
pub fn find_existing(instances: &[Instance]) -> Vec<ExistingGame> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet),
    );
    let mut found = Vec::new();
    for (pid, proc) in sys.processes() {
        let cmd = proc.cmd();
        let Some(game_dir) = arg_after(cmd, "--gameDir") else { continue };
        let game_dir = PathBuf::from(game_dir);
        let Some(inst) = instances.iter().find(|i| same_path(&i.game_dir(), &game_dir)) else { continue };
        let heap_mb = cmd
            .iter()
            .filter_map(|a| a.to_str()?.strip_prefix("-Xmx")?.strip_suffix('M')?.parse::<u64>().ok())
            .next()
            .unwrap_or(inst.memory_mb);
        found.push(ExistingGame { instance_id: inst.id.clone(), pid: pid.as_u32(), heap_mb });
    }
    found
}

fn arg_after(cmd: &[OsString], flag: &str) -> Option<String> {
    let i = cmd.iter().position(|a| a == flag)?;
    cmd.get(i + 1)?.to_str().map(str::to_owned)
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Opt-in end-to-end checks against the live services. Run with
    //   EXILE_HOME=<scratch dir> EXILE_SMOKE_VERSION=<id> [EXILE_SMOKE_MODS=sodium,modmenu]
    //   cargo test -- --ignored --test-threads=1 smoke
    // Setting EXILE_SMOKE_MODS also installs Fabric and those Modrinth mods.

    fn smoke_instance() -> Instance {
        let version = std::env::var("EXILE_SMOKE_VERSION").unwrap_or_else(|_| "1.7.10".into());
        let modded = std::env::var("EXILE_SMOKE_MODS").is_ok_and(|m| !m.is_empty());
        let loader = modded.then(|| loader::latest(loader::LoaderKind::Fabric, &version).unwrap());
        Instance {
            id: format!("smoke-{version}"),
            name: "Smoke test".into(),
            version,
            release_time: String::new(),
            memory_mb: 2048,
            created: 0,
            last_played: None,
            loader,
        }
    }

    #[test]
    #[ignore]
    fn smoke_1_install() {
        let mut inst = smoke_instance();
        let entry = versions::load_manifest().unwrap().into_iter().find(|v| v.id == inst.version).expect("unknown version");
        inst.release_time = entry.release_time.clone();
        install::install(&entry, inst.loader.as_ref(), &crate::net::Progress::default()).unwrap();
        assert!(install::is_complete(&inst.version, inst.loader.as_ref()));
        inst.save().unwrap();

        let Some(spec) = &inst.loader else { return };
        let target = crate::mods::Target { game: inst.version.clone(), loader: spec.kind, curseforge_key: None };
        let mut list = crate::mods::ModList::default();
        for slug in std::env::var("EXILE_SMOKE_MODS").unwrap().split(',') {
            let hit = crate::mods::search(crate::mods::Source::Modrinth, slug, &target)
                .unwrap()
                .into_iter()
                .find(|h| h.slug == slug)
                .unwrap_or_else(|| panic!("{slug} not found for {}", inst.version));
            let out = crate::mods::install(&hit, &target, &list, &inst.mods_dir(), &crate::net::Progress::default()).unwrap();
            assert!(out.warnings.is_empty(), "{:?}", out.warnings);
            list.merge(&inst.mods_dir(), out.installed);
        }
        list.save(&inst).unwrap();
        let views = crate::mods::scan(&inst, &list);
        assert!(views.iter().all(|v| v.issues.is_empty()), "{views:?}");
        eprintln!("installed: {:?}", views.iter().map(|v| v.title().to_owned()).collect::<Vec<_>>());
    }

    /// Builds the real command line and makes sure the game is still running
    /// after 25 seconds.
    #[test]
    #[ignore]
    fn smoke_2_launch() {
        let inst = Instance { loader: crate::instance::load_all().into_iter().find(|i| i.id == smoke_instance().id).and_then(|i| i.loader), ..smoke_instance() };
        assert!(install::is_complete(&inst.version, inst.loader.as_ref()), "not installed under {}", paths::root().display());
        let auth = LaunchAuth {
            name: "Smoke".into(),
            uuid: "00000000000000000000000000000000".into(),
            access_token: "0".into(),
        };
        let mut child = spawn(&inst, 2048, &auth).expect("spawn");
        std::thread::sleep(std::time::Duration::from_secs(25));
        let status = child.try_wait().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        let log = fs::read_to_string(inst.output_log()).unwrap_or_default();
        assert!(status.is_none(), "game exited early with {status:?}\n{log}");
    }

    #[test]
    fn substitutes_placeholders() {
        let vars = HashMap::from([("a", "1".to_owned()), ("b", "${a}".to_owned())]);
        assert_eq!(substitute("x=${a},y=${a}", &vars), "x=1,y=1");
        assert_eq!(substitute("${missing}", &vars), "");
        assert_eq!(substitute("${b}", &vars), "${a}");
        assert_eq!(substitute("plain", &vars), "plain");
    }
}

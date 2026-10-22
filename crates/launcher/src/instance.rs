//! Instances and launcher settings, persisted as TOML.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::loader::LoaderSpec;
use crate::paths;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Instance {
    #[serde(skip)]
    pub id: String,
    pub name: String,
    pub version: String,
    /// Release time of `version`, kept so memory needs are known offline.
    pub release_time: String,
    /// Heap size in MB. Always passed through `memory::sanitize` before use.
    pub memory_mb: u64,
    pub created: u64,
    #[serde(default)]
    pub last_played: Option<u64>,
    /// Mod loader, if mods are enabled. Kept last: TOML tables must follow
    /// plain values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader: Option<LoaderSpec>,
}

pub fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Instance {
    pub fn dir(&self) -> PathBuf {
        paths::instances_dir().join(&self.id)
    }

    pub fn game_dir(&self) -> PathBuf {
        self.dir().join("minecraft")
    }

    pub fn natives_dir(&self) -> PathBuf {
        self.dir().join("natives")
    }

    /// stdout/stderr of the most recent session.
    pub fn output_log(&self) -> PathBuf {
        self.dir().join("output.log")
    }

    /// Where the game (via the mod loader) looks for mod jars.
    pub fn mods_dir(&self) -> PathBuf {
        self.game_dir().join("mods")
    }

    /// Exile's record of which mods it installed and where they came from.
    pub fn mods_manifest(&self) -> PathBuf {
        self.dir().join("mods.toml")
    }

    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(self.game_dir())?;
        let text = toml::to_string_pretty(self)?;
        let path = self.dir().join("instance.toml");
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &path).context("saving instance")
    }

    pub fn delete(&self) -> Result<()> {
        fs::remove_dir_all(self.dir()).with_context(|| format!("deleting {}", self.dir().display()))
    }
}

/// Folder-safe id derived from the name, made unique against existing ones.
pub fn new_id(name: &str, existing: &[Instance]) -> String {
    let mut base: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    base = base.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    base.truncate(40);
    if base.is_empty() {
        base = "instance".into();
    }
    let taken = |id: &str| existing.iter().any(|i| i.id == id) || paths::instances_dir().join(id).exists();
    if !taken(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}-{n}")).find(|id| !taken(id)).unwrap_or(base)
}

pub fn load_all() -> Vec<Instance> {
    let Ok(entries) = fs::read_dir(paths::instances_dir()) else { return Vec::new() };
    let mut out: Vec<Instance> = entries
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_str()?.to_owned();
            let text = fs::read_to_string(e.path().join("instance.toml")).ok()?;
            let mut inst: Instance = toml::from_str(&text).ok()?;
            inst.id = id;
            Some(inst)
        })
        .collect();
    // Most recently played (or created) first.
    out.sort_by_key(|i| std::cmp::Reverse(i.last_played.unwrap_or(i.created)));
    out
}

/// Application (client) ID of Exile's Microsoft app registration. Public by
/// design: it identifies the launcher to Microsoft and grants nothing.
const EXILE_CLIENT_ID: &str = "5365d536-555e-4574-aba0-7e5f18c07e2e";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Azure application (client) ID used for Microsoft sign-in.
    #[serde(default)]
    pub microsoft_client_id: Option<String>,
    #[serde(default)]
    pub show_snapshots: bool,
    #[serde(default)]
    pub selected: Option<String>,
    /// CurseForge API key (from console.curseforge.com). CurseForge search
    /// stays hidden until one is set here, in `EXILE_CURSEFORGE_KEY`, or at
    /// build time.
    #[serde(default)]
    pub curseforge_api_key: Option<String>,
}

impl Settings {
    pub fn load() -> Settings {
        fs::read_to_string(paths::settings_file())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = fs::create_dir_all(paths::root());
        if let Ok(text) = toml::to_string_pretty(self) {
            let _ = fs::write(paths::settings_file(), text);
        }
    }

    /// Exile's own Azure app registration, used unless overridden by
    /// settings.toml or the `EXILE_MSA_CLIENT_ID` environment variable
    /// (handy for testing a different registration).
    pub fn client_id(&self) -> Option<String> {
        self.microsoft_client_id
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| std::env::var("EXILE_MSA_CLIENT_ID").ok().filter(|s| !s.trim().is_empty()))
            .map(|s| s.trim().to_owned())
            .or_else(|| Some(EXILE_CLIENT_ID.to_owned()))
    }

    pub fn curseforge_key(&self) -> Option<String> {
        self.curseforge_api_key
            .clone()
            .or_else(|| std::env::var("EXILE_CURSEFORGE_KEY").ok())
            .or_else(|| option_env!("EXILE_CURSEFORGE_KEY").map(str::to_owned))
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    }
}

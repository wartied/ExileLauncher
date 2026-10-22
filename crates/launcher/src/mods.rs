//! Mods: searching Modrinth and CurseForge, installing a mod together with
//! its required dependencies, and the per-instance list of installed mods.
//!
//! Mods live in `<instance>/minecraft/mods`. Disabled mods are renamed to
//! `*.jar.disabled`, the convention every launcher and loader understands.
//! `<instance>/mods.toml` remembers where each file came from, so
//! dependencies and conflicts can be shown and cleaned up.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::instance::Instance;
use crate::loader::LoaderKind;
use crate::net::{self, Download, Progress, url_encode};

const MODRINTH: &str = "https://api.modrinth.com/v2";
const CURSEFORGE: &str = "https://api.curseforge.com/v1";
const CF_MINECRAFT: u32 = 432;
const CF_CLASS_MODS: u32 = 6;
const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Modrinth,
    CurseForge,
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::Modrinth => "Modrinth",
            Source::CurseForge => "CurseForge",
        }
    }
}

/// What the mods must be compatible with.
#[derive(Clone, Debug)]
pub struct Target {
    pub game: String,
    pub loader: LoaderKind,
    pub curseforge_key: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub source: Source,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub downloads: u64,
}

/// A dependency or conflict, remembered by id with a readable name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModRef {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledMod {
    pub source: Source,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub version: String,
    /// Jar file name in the mods folder (without `.disabled`).
    pub file: String,
    /// Chosen by the user, as opposed to pulled in as a dependency.
    pub explicit: bool,
    #[serde(default)]
    pub requires: Vec<ModRef>,
    #[serde(default)]
    pub incompatible: Vec<ModRef>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModList {
    #[serde(default, rename = "mod")]
    pub mods: Vec<InstalledMod>,
}

impl ModList {
    pub fn load(inst: &Instance) -> ModList {
        fs::read_to_string(inst.mods_manifest())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, inst: &Instance) -> Result<()> {
        let path = inst.mods_manifest();
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, toml::to_string_pretty(self)?)?;
        fs::rename(&tmp, &path).context("saving mod list")
    }

    pub fn contains(&self, source: Source, project_id: &str) -> bool {
        self.mods.iter().any(|m| m.source == source && m.project_id == project_id)
    }

    /// Adds freshly installed mods, replacing older versions of the same
    /// project (whose jar is deleted if the file name changed).
    pub fn merge(&mut self, mods_dir: &Path, new: Vec<InstalledMod>) {
        for m in new {
            if let Some(old) = self.mods.iter_mut().find(|o| o.source == m.source && o.project_id == m.project_id) {
                if old.file != m.file {
                    let _ = fs::remove_file(mods_dir.join(&old.file));
                    let _ = fs::remove_file(mods_dir.join(format!("{}{DISABLED_SUFFIX}", old.file)));
                }
                // Something the user picked stays "picked" even if it is
                // later reinstalled as another mod's dependency.
                let explicit = old.explicit || m.explicit;
                *old = InstalledMod { explicit, ..m };
            } else {
                self.mods.push(m);
            }
        }
        self.mods.sort_by_key(|m| m.title.to_lowercase());
    }
}

// ---------------------------------------------------------------------------
// What's on disk

/// One row of the installed-mods list.
#[derive(Clone, Debug)]
pub struct ModView {
    /// Jar file name without `.disabled`.
    pub file: String,
    pub enabled: bool,
    /// Present when Exile installed it; `None` for jars added by hand.
    pub entry: Option<InstalledMod>,
    pub issues: Vec<String>,
}

impl ModView {
    pub fn title(&self) -> &str {
        self.entry.as_ref().map(|e| e.title.as_str()).unwrap_or(&self.file)
    }
}

/// Lists the mods folder, matched against Exile's records, with issues
/// (conflicts, missing or disabled dependencies) worked out per mod.
pub fn scan(inst: &Instance, list: &ModList) -> Vec<ModView> {
    let mut on_disk: Vec<(String, bool)> = fs::read_dir(inst.mods_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            if let Some(base) = name.strip_suffix(DISABLED_SUFFIX) {
                base.ends_with(".jar").then(|| (base.to_owned(), false))
            } else {
                name.ends_with(".jar").then_some((name, true))
            }
        })
        .collect();
    on_disk.sort_by_key(|(f, _)| f.to_lowercase());

    let mut views: Vec<ModView> = on_disk
        .into_iter()
        .map(|(file, enabled)| ModView {
            entry: list.mods.iter().find(|m| m.file == file).cloned(),
            file,
            enabled,
            issues: Vec::new(),
        })
        .collect();
    views.sort_by_key(|v| v.title().to_lowercase());

    let enabled_ids: HashSet<String> =
        views.iter().filter(|v| v.enabled).filter_map(|v| v.entry.as_ref().map(|e| e.project_id.clone())).collect();
    let all_ids: HashSet<String> = views.iter().filter_map(|v| v.entry.as_ref().map(|e| e.project_id.clone())).collect();
    let titles: HashMap<String, String> =
        views.iter().filter_map(|v| v.entry.as_ref().map(|e| (e.project_id.clone(), e.title.clone()))).collect();

    for v in views.iter_mut().filter(|v| v.enabled) {
        let Some(e) = &v.entry else { continue };
        for dep in &e.requires {
            if !all_ids.contains(&dep.id) {
                v.issues.push(format!("Needs {}", dep.title));
            } else if !enabled_ids.contains(&dep.id) {
                v.issues.push(format!("Needs {} (disabled)", dep.title));
            }
        }
        for bad in &e.incompatible {
            if enabled_ids.contains(&bad.id) {
                let name = titles.get(&bad.id).unwrap_or(&bad.title);
                v.issues.push(format!("Conflicts with {name}"));
            }
        }
    }
    views
}

pub fn set_enabled(inst: &Instance, file: &str, enabled: bool) -> Result<()> {
    let dir = inst.mods_dir();
    let on = dir.join(file);
    let off = dir.join(format!("{file}{DISABLED_SUFFIX}"));
    let (from, to) = if enabled { (off, on) } else { (on, off) };
    fs::rename(&from, &to).with_context(|| format!("renaming {file}"))
}

/// Deletes a mod, then any dependency Exile pulled in that nothing still
/// needs. Returns the titles of the extra mods removed.
pub fn remove(inst: &Instance, list: &mut ModList, file: &str) -> Result<Vec<String>> {
    let dir = inst.mods_dir();
    for name in [file.to_owned(), format!("{file}{DISABLED_SUFFIX}")] {
        let path = dir.join(&name);
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("deleting {name}"))?;
        }
    }
    list.mods.retain(|m| m.file != file);

    let mut removed = Vec::new();
    loop {
        let needed: HashSet<&str> = list.mods.iter().flat_map(|m| m.requires.iter().map(|d| d.id.as_str())).collect();
        let Some(pos) = list.mods.iter().position(|m| !m.explicit && !needed.contains(m.project_id.as_str())) else {
            break;
        };
        let orphan = list.mods.remove(pos);
        let _ = fs::remove_file(dir.join(&orphan.file));
        let _ = fs::remove_file(dir.join(format!("{}{DISABLED_SUFFIX}", orphan.file)));
        removed.push(orphan.title);
    }
    Ok(removed)
}

// ---------------------------------------------------------------------------
// Hardware hints

/// Mods known to lean hard on the GPU or memory.
const DEMANDING: &[(&str, &str)] = &[
    ("iris", "Shaders need a dedicated graphics card to run smoothly."),
    ("oculus", "Shaders need a dedicated graphics card to run smoothly."),
    ("distanthorizons", "Distant Horizons needs a strong graphics card and extra memory."),
];

/// True for integrated or software renderers (from the OpenGL renderer string).
pub fn weak_gpu(renderer: &str) -> bool {
    let r = renderer.to_lowercase();
    ["intel", "basic render", "llvmpipe", "softpipe", "radeon(tm) graphics", "radeon graphics", "vega 8", "vega 7"]
        .iter()
        .any(|k| r.contains(k))
}

pub fn demanding_note(slug: &str) -> Option<&'static str> {
    DEMANDING.iter().find(|(s, _)| *s == slug).map(|(_, why)| *why)
}

// ---------------------------------------------------------------------------
// Search

pub fn search(source: Source, query: &str, target: &Target) -> Result<Vec<SearchHit>> {
    match source {
        Source::Modrinth => modrinth_search(query, target),
        Source::CurseForge => curseforge_search(query, target),
    }
}

#[derive(Deserialize)]
struct MrSearch {
    hits: Vec<MrHit>,
}

#[derive(Deserialize)]
struct MrHit {
    project_id: String,
    slug: String,
    title: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    downloads: u64,
}

fn modrinth_search(query: &str, t: &Target) -> Result<Vec<SearchHit>> {
    let facets = format!(r#"[["project_type:mod"],["versions:{}"],["categories:{}"]]"#, t.game, t.loader.slug());
    let index = if query.trim().is_empty() { "downloads" } else { "relevance" };
    let url = format!(
        "{MODRINTH}/search?query={}&limit=30&index={index}&facets={}",
        url_encode(query.trim()),
        url_encode(&facets)
    );
    let res: MrSearch = net::get_json(&url).context("searching Modrinth")?;
    Ok(res
        .hits
        .into_iter()
        .map(|h| SearchHit {
            source: Source::Modrinth,
            project_id: h.project_id,
            slug: h.slug,
            title: h.title,
            author: h.author,
            description: h.description,
            downloads: h.downloads,
        })
        .collect())
}

fn cf_get(url: &str, key: &str) -> Result<Value> {
    let mut resp = net::agent()
        .get(url)
        .header("x-api-key", key)
        .header("Accept", "application/json")
        .call()
        .context("could not reach CurseForge")?;
    match resp.status().as_u16() {
        200 => {}
        403 => bail!("CurseForge rejected the API key."),
        s => bail!("CurseForge returned HTTP {s}."),
    }
    let body = resp.body_mut().with_config().limit(16 * 1024 * 1024).read_to_vec()?;
    Ok(serde_json::from_slice(&body)?)
}

/// CurseForge's numeric loader ids.
fn cf_loader(kind: LoaderKind) -> u32 {
    match kind {
        LoaderKind::Fabric => 4,
    }
}

fn cf_key(t: &Target) -> Result<&str> {
    t.curseforge_key.as_deref().ok_or_else(|| anyhow!("CurseForge needs an API key."))
}

fn str_of(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn curseforge_search(query: &str, t: &Target) -> Result<Vec<SearchHit>> {
    let key = cf_key(t)?;
    let url = format!(
        "{CURSEFORGE}/mods/search?gameId={CF_MINECRAFT}&classId={CF_CLASS_MODS}&searchFilter={}&gameVersion={}\
         &modLoaderType={}&sortField=2&sortOrder=desc&pageSize=30",
        url_encode(query.trim()),
        url_encode(&t.game),
        cf_loader(t.loader)
    );
    let res = cf_get(&url, key)?;
    let data = res.get("data").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(data
        .iter()
        .map(|m| SearchHit {
            source: Source::CurseForge,
            project_id: str_of(m, "id"),
            slug: str_of(m, "slug"),
            title: str_of(m, "name"),
            author: m.pointer("/authors/0/name").and_then(Value::as_str).unwrap_or("").to_owned(),
            description: str_of(m, "summary"),
            downloads: m.get("downloadCount").and_then(Value::as_f64).unwrap_or(0.0) as u64,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Install

/// One file to download plus the record to keep once it's on disk.
struct Planned {
    entry: InstalledMod,
    download: Download,
}

pub struct InstallOutcome {
    pub installed: Vec<InstalledMod>,
    /// Things the user should know, e.g. a dependency that isn't available.
    pub warnings: Vec<String>,
}

/// Installs a mod and every required dependency not already installed.
pub fn install(
    hit: &SearchHit,
    target: &Target,
    already: &ModList,
    mods_dir: &Path,
    progress: &Progress,
) -> Result<InstallOutcome> {
    progress.set_stage(format!("Resolving {}", hit.title));
    let mut warnings = Vec::new();
    let mut plan: Vec<Planned> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // (project id, title if known, explicit, required by)
    let mut queue: VecDeque<(String, Option<String>, bool, Option<String>)> = VecDeque::new();
    queue.push_back((hit.project_id.clone(), Some(hit.title.clone()), true, None));

    while let Some((id, title, explicit, parent)) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        // Dependencies already present are left alone; the mod the user
        // picked is always (re)installed so it ends up current.
        if !explicit && already.contains(hit.source, &id) {
            continue;
        }
        let resolved = match hit.source {
            Source::Modrinth => modrinth_resolve(&id, title, explicit, target),
            Source::CurseForge => curseforge_resolve(&id, title, explicit, target),
        };
        match resolved {
            Ok(mut p) => {
                p.download.path = mods_dir.join(&p.entry.file);
                for dep in &p.entry.requires {
                    queue.push_back((dep.id.clone(), Some(dep.title.clone()), false, Some(p.entry.title.clone())));
                }
                plan.push(p);
            }
            Err(e) if explicit => return Err(e),
            Err(e) => {
                let who = parent.unwrap_or_else(|| hit.title.clone());
                warnings.push(format!("{who}: {e:#}"));
            }
        }
    }

    progress.set_stage(format!("Downloading {}", hit.title));
    progress.total_bytes.fetch_add(
        plan.iter().filter_map(|p| p.download.size).sum(),
        std::sync::atomic::Ordering::Relaxed,
    );
    fs::create_dir_all(mods_dir)?;
    for p in &plan {
        if !net::file_matches(&p.download.path, p.download.sha1.as_deref(), p.download.size) {
            net::download(&p.download, progress)?;
        }
    }
    Ok(InstallOutcome { installed: plan.into_iter().map(|p| p.entry).collect(), warnings })
}

/// Jar names come from remote metadata: keep them to a plain file name.
fn safe_file_name(name: &str) -> Result<String> {
    let ok = !name.is_empty()
        && name.ends_with(".jar")
        && !name.contains(['/', '\\', ':'])
        && !name.starts_with('.');
    if ok { Ok(name.to_owned()) } else { Err(anyhow!("unexpected file name {name:?}")) }
}

#[derive(Deserialize)]
struct MrVersion {
    name: String,
    version_number: String,
    #[serde(default)]
    version_type: String,
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: Vec<MrDep>,
}

#[derive(Deserialize)]
struct MrFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    size: Option<u64>,
    hashes: HashMap<String, String>,
}

#[derive(Deserialize)]
struct MrDep {
    project_id: Option<String>,
    version_id: Option<String>,
    dependency_type: String,
}

#[derive(Deserialize)]
struct MrProject {
    id: String,
    slug: String,
    title: String,
}

#[derive(Deserialize)]
struct MrVersionRef {
    project_id: String,
}

fn modrinth_project(id: &str) -> Result<MrProject> {
    net::get_json(&format!("{MODRINTH}/project/{}", url_encode(id)))
}

fn modrinth_resolve(id: &str, title: Option<String>, explicit: bool, t: &Target) -> Result<Planned> {
    let project = modrinth_project(id).with_context(|| format!("looking up {}", title.as_deref().unwrap_or(id)))?;
    let url = format!(
        "{MODRINTH}/project/{}/version?loaders={}&game_versions={}",
        url_encode(&project.id),
        url_encode(&format!(r#"["{}"]"#, t.loader.slug())),
        url_encode(&format!(r#"["{}"]"#, t.game)),
    );
    let versions: Vec<MrVersion> = net::get_json(&url)?;
    // Newest stable release, else the newest of anything.
    let version = versions
        .iter()
        .find(|v| v.version_type == "release")
        .or_else(|| versions.first())
        .ok_or_else(|| anyhow!("{} isn't available for Minecraft {} with {}", project.title, t.game, t.loader.name()))?;
    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| anyhow!("{} has no downloadable file", project.title))?;
    let file_name = safe_file_name(&file.filename)?;

    let mut requires = Vec::new();
    let mut incompatible = Vec::new();
    for dep in &version.dependencies {
        let kind = dep.dependency_type.as_str();
        if kind != "required" && kind != "incompatible" {
            continue;
        }
        let dep_id = match (&dep.project_id, &dep.version_id) {
            (Some(p), _) => p.clone(),
            (None, Some(v)) => {
                let r: MrVersionRef = net::get_json(&format!("{MODRINTH}/version/{}", url_encode(v)))?;
                r.project_id
            }
            _ => continue,
        };
        let dep_title = modrinth_project(&dep_id).map(|p| p.title).unwrap_or_else(|_| dep_id.clone());
        let r = ModRef { id: dep_id, title: dep_title };
        if kind == "required" { requires.push(r) } else { incompatible.push(r) }
    }

    Ok(Planned {
        download: Download {
            url: file.url.clone(),
            path: PathBuf::new(), // set by `install`
            sha1: file.hashes.get("sha1").cloned(),
            size: file.size,
        },
        entry: InstalledMod {
            source: Source::Modrinth,
            project_id: project.id,
            slug: project.slug,
            title: project.title,
            version: if version.version_number.is_empty() { version.name.clone() } else { version.version_number.clone() },
            file: file_name,
            explicit,
            requires,
            incompatible,
        },
    })
}

fn curseforge_resolve(id: &str, title: Option<String>, explicit: bool, t: &Target) -> Result<Planned> {
    let key = cf_key(t)?;
    let project = cf_get(&format!("{CURSEFORGE}/mods/{}", url_encode(id)), key)?;
    let project = project.get("data").cloned().unwrap_or(Value::Null);
    let name = str_of(&project, "name");
    let name = if name.is_empty() { title.unwrap_or_else(|| id.to_owned()) } else { name };

    let url = format!(
        "{CURSEFORGE}/mods/{}/files?gameVersion={}&modLoaderType={}&pageSize=50",
        url_encode(id),
        url_encode(&t.game),
        cf_loader(t.loader)
    );
    let files = cf_get(&url, key)?;
    let mut files = files.get("data").and_then(Value::as_array).cloned().unwrap_or_default();
    // Newest first; releaseType 1 = release.
    files.sort_by_key(|f| std::cmp::Reverse(str_of(f, "fileDate")));
    let file = files
        .iter()
        .find(|f| f.get("releaseType").and_then(Value::as_u64) == Some(1))
        .or_else(|| files.first())
        .ok_or_else(|| anyhow!("{name} isn't available for Minecraft {} with {}", t.game, t.loader.name()))?;
    let download_url = file.get("downloadUrl").and_then(Value::as_str).ok_or_else(|| {
        anyhow!("{name}'s author only allows downloads from the CurseForge website.")
    })?;
    let file_name = safe_file_name(&str_of(file, "fileName"))?;
    let sha1 = file
        .get("hashes")
        .and_then(Value::as_array)
        .and_then(|h| h.iter().find(|h| h.get("algo").and_then(Value::as_u64) == Some(1)))
        .map(|h| str_of(h, "value"));

    let mut requires = Vec::new();
    let mut incompatible = Vec::new();
    for dep in file.get("dependencies").and_then(Value::as_array).into_iter().flatten() {
        let dep_id = str_of(dep, "modId");
        let relation = dep.get("relationType").and_then(Value::as_u64);
        if relation != Some(3) && relation != Some(5) {
            continue;
        }
        let dep_title = cf_get(&format!("{CURSEFORGE}/mods/{dep_id}"), key)
            .ok()
            .and_then(|v| v.pointer("/data/name").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| dep_id.clone());
        let r = ModRef { id: dep_id, title: dep_title };
        if relation == Some(3) { requires.push(r) } else { incompatible.push(r) }
    }

    Ok(Planned {
        download: Download {
            url: download_url.to_owned(),
            path: PathBuf::new(), // set by `install`
            sha1,
            size: file.get("fileLength").and_then(Value::as_u64),
        },
        entry: InstalledMod {
            source: Source::CurseForge,
            project_id: id.to_owned(),
            slug: str_of(&project, "slug"),
            title: name,
            version: str_of(file, "displayName"),
            file: file_name,
            explicit,
            requires,
            incompatible,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, file: &str, explicit: bool, requires: &[&str]) -> InstalledMod {
        InstalledMod {
            source: Source::Modrinth,
            project_id: id.into(),
            slug: id.into(),
            title: id.into(),
            version: "1".into(),
            file: file.into(),
            explicit,
            requires: requires.iter().map(|r| ModRef { id: (*r).into(), title: (*r).into() }).collect(),
            incompatible: vec![],
        }
    }

    #[test]
    fn rejects_unsafe_file_names() {
        assert!(safe_file_name("sodium-0.6.jar").is_ok());
        assert!(safe_file_name("../evil.jar").is_err());
        assert!(safe_file_name("C:evil.jar").is_err());
        assert!(safe_file_name("notes.txt").is_err());
    }

    #[test]
    fn merge_replaces_versions_and_keeps_explicit() {
        let dir = std::env::temp_dir();
        let mut list = ModList::default();
        list.merge(&dir, vec![m("api", "api-1.jar", true, &[])]);
        list.merge(&dir, vec![m("api", "api-2.jar", false, &[])]);
        assert_eq!(list.mods.len(), 1);
        assert_eq!(list.mods[0].file, "api-2.jar");
        assert!(list.mods[0].explicit);
    }

    #[test]
    fn weak_gpus() {
        assert!(weak_gpu("Intel(R) UHD Graphics 620"));
        assert!(weak_gpu("AMD Radeon(TM) Graphics"));
        assert!(!weak_gpu("NVIDIA GeForce RTX 4070/PCIe/SSE2"));
    }
}

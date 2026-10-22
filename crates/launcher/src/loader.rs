//! Mod loader support. Fabric only for now: it covers the large majority of
//! performance mods, installs from a single JSON profile, and runs on
//! Minecraft 1.14 and newer.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::net::{self, Download};
use crate::paths;
use crate::versions::{Arguments, Library, VersionJson};

const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
/// Release time of 1.14, the first version Fabric supports.
const FABRIC_OLDEST: &str = "2019-04-23";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Fabric,
}

impl LoaderKind {
    pub fn name(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "Fabric",
        }
    }

    /// Identifier used by mod sites (Modrinth loader facet).
    pub fn slug(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "fabric",
        }
    }

    pub fn supports(self, release_time: &str) -> bool {
        match self {
            LoaderKind::Fabric => release_time >= FABRIC_OLDEST,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderSpec {
    pub kind: LoaderKind,
    pub version: String,
}

impl LoaderSpec {
    pub fn label(&self) -> String {
        format!("{} {}", self.kind.name(), self.version)
    }

    fn profile_id(&self, game: &str) -> String {
        format!("fabric-loader-{}-{game}", self.version)
    }

    fn profile_path(&self, game: &str) -> PathBuf {
        let id = self.profile_id(game);
        paths::version_dir(&id).join(format!("{id}.json"))
    }
}

#[derive(Deserialize)]
struct LoaderEntry {
    loader: LoaderVersion,
}

#[derive(Deserialize)]
struct LoaderVersion {
    version: String,
    #[serde(default)]
    stable: bool,
}

/// Newest stable Fabric loader for a game version.
pub fn latest(kind: LoaderKind, game: &str) -> Result<LoaderSpec> {
    match kind {
        LoaderKind::Fabric => {
            let list: Vec<LoaderEntry> = net::get_json(&format!("{FABRIC_META}/versions/loader/{game}"))?;
            let pick = list
                .iter()
                .find(|e| e.loader.stable)
                .or_else(|| list.first())
                .ok_or_else(|| anyhow!("Fabric isn't available for Minecraft {game} yet."))?;
            Ok(LoaderSpec { kind, version: pick.loader.version.clone() })
        }
    }
}

/// The part of a loader profile Exile uses. It "inherits from" the vanilla
/// version and adds libraries, a main class and arguments.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderProfile {
    pub main_class: String,
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub libraries: Vec<Library>,
}

pub fn load_profile(spec: &LoaderSpec, game: &str) -> Result<LoaderProfile> {
    let path = spec.profile_path(game);
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).context("parsing loader profile")
}

/// Fetches the loader profile if needed and returns the library downloads.
pub fn plan(spec: &LoaderSpec, game: &str) -> Result<Vec<Download>> {
    let path = spec.profile_path(game);
    if load_profile(spec, game).is_err() {
        let url = format!("{FABRIC_META}/versions/loader/{game}/{}/profile/json", spec.version);
        let bytes = net::get_bytes(&url).context("downloading Fabric profile")?;
        // Parse before saving so a bad response is never cached.
        serde_json::from_slice::<LoaderProfile>(&bytes).context("parsing Fabric profile")?;
        fs::create_dir_all(path.parent().unwrap_or(&paths::versions_dir()))?;
        fs::write(&path, bytes)?;
    }
    downloads(&load_profile(spec, game)?)
}

fn downloads(profile: &LoaderProfile) -> Result<Vec<Download>> {
    let mut out = Vec::new();
    for lib in profile.libraries.iter().filter(|l| l.allowed()) {
        let Some(art) = lib.artifact() else { continue };
        let Some(rel) = art.path.as_deref() else { continue };
        let path = paths::safe_join(&paths::libraries_dir(), rel).ok_or_else(|| anyhow!("unsafe library path {rel}"))?;
        out.push(Download { url: art.url, path, sha1: art.sha1, size: art.size });
    }
    Ok(out)
}

pub fn is_complete(spec: &LoaderSpec, game: &str) -> bool {
    load_profile(spec, game)
        .and_then(|p| downloads(&p))
        .is_ok_and(|d| d.iter().all(|d| net::file_present(&d.path, d.size)))
}

/// Merges a loader profile into the vanilla version. Loader libraries come
/// first and win over vanilla ones with the same `group:artifact`, which is
/// how the official launcher resolves `inheritsFrom`.
pub fn apply(mut v: VersionJson, profile: LoaderProfile) -> VersionJson {
    let loader_keys: HashSet<String> = profile.libraries.iter().map(Library::key).collect();
    let mut libraries = profile.libraries;
    libraries.extend(v.libraries.into_iter().filter(|l| !loader_keys.contains(&l.key())));
    v.libraries = libraries;
    v.main_class = profile.main_class;
    if let Some(extra) = profile.arguments {
        match &mut v.arguments {
            Some(args) => {
                args.jvm.extend(extra.jvm);
                args.game.extend(extra.game);
            }
            None => v.arguments = Some(extra),
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabric_version_range() {
        assert!(LoaderKind::Fabric.supports("2024-08-08T12:24:45+00:00"));
        assert!(!LoaderKind::Fabric.supports("2014-05-14T17:29:23+00:00"));
    }

    #[test]
    fn library_keys() {
        let lib: Library = serde_json::from_str(r#"{"name":"org.ow2.asm:asm:9.7","url":"https://maven.fabricmc.net/"}"#).unwrap();
        assert_eq!(lib.key(), "org.ow2.asm:asm");
        let art = lib.artifact().unwrap();
        assert_eq!(art.url, "https://maven.fabricmc.net/org/ow2/asm/asm/9.7/asm-9.7.jar");
    }
}

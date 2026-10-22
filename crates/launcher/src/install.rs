//! Installing a vanilla version: version JSON, client jar, libraries, assets,
//! logging config and the matching Java runtime. Files are shared between
//! instances; each is verified by SHA-1 when downloaded.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result};

use crate::loader::{self, LoaderSpec};
use crate::net::{self, Download, Progress};
use crate::versions::{self, AssetIndex, ManifestEntry, VersionJson};
use crate::{java, paths};

const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

fn asset_index_path(v: &VersionJson) -> PathBuf {
    paths::assets_dir().join("indexes").join(format!("{}.json", v.asset_index.id))
}

fn asset_object_path(hash: &str) -> PathBuf {
    paths::assets_dir().join("objects").join(&hash[..2]).join(hash)
}

pub fn log_config_path(id: &str) -> PathBuf {
    paths::assets_dir().join("log_configs").join(id)
}

pub fn virtual_assets_dir(v: &VersionJson) -> PathBuf {
    paths::assets_dir().join("virtual").join(&v.asset_index.id)
}

fn library_path(rel: &str) -> Option<PathBuf> {
    paths::safe_join(&paths::libraries_dir(), rel)
}

fn load_asset_index(v: &VersionJson) -> Result<AssetIndex> {
    let bytes = fs::read(asset_index_path(v))?;
    serde_json::from_slice(&bytes).context("parsing asset index")
}

/// Every file the game needs apart from assets and Java.
fn game_files(v: &VersionJson) -> Result<Vec<Download>> {
    let mut out = vec![Download {
        url: v.downloads.client.url.clone(),
        path: paths::version_jar(&v.id),
        sha1: v.downloads.client.sha1.clone(),
        size: v.downloads.client.size,
    }];
    for lib in v.libraries.iter().filter(|l| l.allowed()) {
        for art in [lib.artifact(), lib.native_artifact()].into_iter().flatten() {
            let Some(rel) = art.path.as_deref() else { continue };
            if art.url.is_empty() {
                continue;
            }
            let path = library_path(rel).with_context(|| format!("unsafe library path {rel}"))?;
            out.push(Download { url: art.url.clone(), path, sha1: art.sha1.clone(), size: art.size });
        }
    }
    if let Some(log) = v.logging.as_ref().and_then(|l| l.client.as_ref()) {
        out.push(Download {
            url: log.file.url.clone(),
            path: log_config_path(&log.file.id),
            sha1: Some(log.file.sha1.clone()),
            size: Some(log.file.size),
        });
    }
    Ok(out)
}

fn asset_files(index: &AssetIndex) -> Vec<Download> {
    let mut seen = HashSet::new();
    index
        .objects
        .values()
        .filter(|o| o.hash.len() == 40 && seen.insert(o.hash.as_str()))
        .map(|o| Download {
            url: format!("{RESOURCES_URL}/{}/{}", &o.hash[..2], o.hash),
            path: asset_object_path(&o.hash),
            sha1: Some(o.hash.clone()),
            size: Some(o.size),
        })
        .collect()
}

/// Installs (or repairs) a version. Safe to call on an installed version:
/// valid files are left alone.
pub fn install(entry: &ManifestEntry, loader: Option<&LoaderSpec>, progress: &Progress) -> Result<()> {
    progress.set_stage("Preparing");
    let id = &entry.id;
    let json = Download { url: entry.url.clone(), path: paths::version_json(id), sha1: entry.sha1.clone(), size: None };
    if !net::file_matches(&json.path, json.sha1.as_deref(), None) {
        net::download(&json, progress).context("downloading version info")?;
    }
    let v = versions::load_version_json(id)?;

    let index = Download {
        url: v.asset_index.url.clone(),
        path: asset_index_path(&v),
        sha1: Some(v.asset_index.sha1.clone()),
        size: Some(v.asset_index.size),
    };
    if !net::file_matches(&index.path, index.sha1.as_deref(), index.size) {
        net::download(&index, progress).context("downloading asset index")?;
    }
    let assets = load_asset_index(&v)?;

    let runtime = java::plan(&java::component_for(&v)).context("checking Java runtime")?;

    // One batch, so the progress bar only ever moves forward.
    let mut all = game_files(&v)?;
    all.extend(asset_files(&assets));
    if let Some(spec) = loader {
        all.extend(loader::plan(spec, id).with_context(|| format!("preparing {}", spec.label()))?);
    }
    if let Some(rt) = &runtime {
        all.extend(rt.downloads.iter().cloned());
    }
    progress.done_bytes.store(0, std::sync::atomic::Ordering::Relaxed);
    progress.total_bytes.store(0, std::sync::atomic::Ordering::Relaxed);
    progress.set_stage("Checking files");
    net::download_all(all, progress)?;

    if let Some(rt) = runtime {
        java::finish(rt)?;
    }
    progress.set_stage("Finishing");
    if assets.is_virtual {
        copy_assets(&assets, &virtual_assets_dir(&v))?;
    }
    Ok(())
}

/// Copies assets out of the hashed store into the layout old versions expect
/// (`assets/virtual/legacy`, or `<game dir>/resources`). Hard links are tried
/// first so this costs no extra disk space.
pub fn copy_assets(index: &AssetIndex, dest: &std::path::Path) -> Result<()> {
    for (name, obj) in &index.objects {
        let Some(target) = paths::safe_join(dest, name) else { continue };
        if net::file_present(&target, Some(obj.size)) {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let src = asset_object_path(&obj.hash);
        let _ = fs::remove_file(&target);
        if fs::hard_link(&src, &target).is_err() {
            fs::copy(&src, &target).with_context(|| format!("copying asset {name}"))?;
        }
    }
    Ok(())
}

/// Fast pre-launch check: every file exists with the right size. Hashes were
/// verified at download time, so this only catches deleted or truncated files.
pub fn is_complete(id: &str, loader: Option<&LoaderSpec>) -> bool {
    if loader.is_some_and(|spec| !loader::is_complete(spec, id)) {
        return false;
    }
    let Ok(v) = versions::load_version_json(id) else { return false };
    if !java::is_installed(&java::component_for(&v)) {
        return false;
    }
    let Ok(files) = game_files(&v) else { return false };
    if !files.iter().all(|d| net::file_present(&d.path, d.size)) {
        return false;
    }
    let Ok(index) = load_asset_index(&v) else { return false };
    index.objects.values().all(|o| o.hash.len() == 40 && net::file_present(&asset_object_path(&o.hash), Some(o.size)))
}

pub fn asset_index_for(v: &VersionJson) -> Result<AssetIndex> {
    load_asset_index(v)
}

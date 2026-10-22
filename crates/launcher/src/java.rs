//! Mojang-provided Java runtimes. Every Minecraft version names the runtime
//! it was built for (Java 8 for 1.7–1.16, 16/17/21/25 for later ones), so the
//! user never has to install or pick Java.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result, anyhow};
use serde::Deserialize;

use crate::net::{self, Download};
use crate::paths;
use crate::versions::VersionJson;

const RUNTIMES_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[derive(Deserialize)]
struct RuntimeEntry {
    manifest: ManifestRef,
    version: RuntimeVersion,
}

#[derive(Deserialize)]
struct ManifestRef {
    sha1: String,
    url: String,
}

#[derive(Deserialize)]
struct RuntimeVersion {
    name: String,
}

#[derive(Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}

#[derive(Deserialize)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    executable: bool,
    downloads: Option<RuntimeDownloads>,
    #[cfg_attr(windows, allow(dead_code))]
    target: Option<String>,
}

#[derive(Deserialize)]
struct RuntimeDownloads {
    raw: crate::versions::Artifact,
}

/// Platform keys used by Mojang, best match first.
fn platform_keys() -> &'static [&'static str] {
    if cfg!(all(windows, target_arch = "aarch64")) {
        // jre-legacy has no ARM build; x64 runs under emulation.
        &["windows-arm64", "windows-x64"]
    } else if cfg!(all(windows, target_pointer_width = "64")) {
        &["windows-x64"]
    } else if cfg!(windows) {
        &["windows-x86"]
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        &["mac-os-arm64", "mac-os"]
    } else if cfg!(target_os = "macos") {
        &["mac-os"]
    } else if cfg!(target_pointer_width = "64") {
        &["linux"]
    } else {
        &["linux-i386"]
    }
}

pub fn component_for(version: &VersionJson) -> String {
    version
        .java_version
        .as_ref()
        .map(|j| j.component.clone())
        .unwrap_or_else(|| "jre-legacy".to_owned())
}

fn runtime_dir(component: &str) -> PathBuf {
    paths::runtimes_dir().join(component)
}

fn stamp_file(component: &str) -> PathBuf {
    paths::runtimes_dir().join(format!("{component}.version"))
}

/// Path to the Java executable inside an installed runtime. On Windows this is
/// `javaw.exe`, which does not open a console window.
pub fn java_path(component: &str) -> PathBuf {
    let dir = runtime_dir(component);
    if cfg!(windows) {
        dir.join("bin").join("javaw.exe")
    } else if cfg!(target_os = "macos") {
        dir.join("jre.bundle/Contents/Home/bin/java")
    } else {
        dir.join("bin").join("java")
    }
}

pub fn is_installed(component: &str) -> bool {
    stamp_file(component).is_file() && java_path(component).is_file()
}

/// Work needed to bring a runtime up to date. Built by [`plan`], the downloads
/// are run together with the game's so progress is reported as one figure,
/// then [`finish`] applies permissions and records the installed version.
pub struct RuntimePlan {
    component: String,
    stamp: String,
    pub downloads: Vec<Download>,
    executables: Vec<PathBuf>,
    links: Vec<(PathBuf, Option<String>)>,
}

/// Returns `None` when the runtime is already installed and current.
pub fn plan(component: &str) -> Result<Option<RuntimePlan>> {
    let all: HashMap<String, HashMap<String, Vec<RuntimeEntry>>> = match net::get_json(RUNTIMES_URL) {
        Ok(all) => all,
        // Offline: an already installed runtime is good enough.
        Err(_) if is_installed(component) => return Ok(None),
        Err(e) => return Err(e),
    };
    let entry = platform_keys()
        .iter()
        .filter_map(|key| all.get(*key)?.get(component)?.first())
        .next()
        .ok_or_else(|| anyhow!("Java runtime '{component}' is not available for this platform"))?;

    let stamp = format!("{} {}", entry.version.name, entry.manifest.sha1);
    if is_installed(component) && fs::read_to_string(stamp_file(component)).is_ok_and(|s| s == stamp) {
        return Ok(None);
    }

    let manifest: RuntimeManifest = net::get_json(&entry.manifest.url)?;
    let root = runtime_dir(component);
    let mut plan = RuntimePlan {
        component: component.to_owned(),
        stamp,
        downloads: Vec::new(),
        executables: Vec::new(),
        links: Vec::new(),
    };
    for (rel, file) in &manifest.files {
        let path = paths::safe_join(&root, rel).ok_or_else(|| anyhow!("unsafe path in runtime manifest: {rel}"))?;
        match file.kind.as_str() {
            "directory" => fs::create_dir_all(&path)?,
            "file" => {
                let raw = &file.downloads.as_ref().ok_or_else(|| anyhow!("{rel} has no download"))?.raw;
                plan.downloads.push(Download {
                    url: raw.url.clone(),
                    path: path.clone(),
                    sha1: raw.sha1.clone(),
                    size: raw.size,
                });
                if file.executable {
                    plan.executables.push(path);
                }
            }
            "link" => plan.links.push((path, file.target.clone())),
            _ => {}
        }
    }
    Ok(Some(plan))
}

/// Call after `plan.downloads` have all completed.
pub fn finish(plan: RuntimePlan) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in &plan.executables {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }
        for (path, target) in &plan.links {
            if let Some(target) = target {
                let _ = fs::remove_file(path);
                std::os::unix::fs::symlink(target, path)?;
            }
        }
    }
    #[cfg(not(unix))]
    let _ = (&plan.executables, &plan.links);

    let java = java_path(&plan.component);
    if !java.is_file() {
        return Err(anyhow!("Java runtime installed but {} is missing", java.display()));
    }
    fs::write(stamp_file(&plan.component), &plan.stamp).context("recording Java runtime version")
}

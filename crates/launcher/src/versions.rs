//! Mojang version metadata: the manifest, per-version JSON and the rule system
//! used to pick libraries and arguments per platform.

use std::collections::HashMap;
use std::fs;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::{net, paths};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
/// Release time of 1.7.2, the first 1.7 release. Nothing older is offered.
pub const OLDEST_SUPPORTED: &str = "2013-10-25T13:00:00+00:00";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub versions: Vec<ManifestEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub release_time: String,
    pub sha1: Option<String>,
}

impl ManifestEntry {
    pub fn is_release(&self) -> bool {
        self.kind == "release"
    }
}

/// Fetches the manifest, falling back to the cached copy when offline.
/// Returned entries are newest first and limited to 1.7 and later.
pub fn load_manifest() -> Result<Vec<ManifestEntry>> {
    let cache = paths::meta_dir().join("version_manifest_v2.json");
    let manifest: Manifest = match net::get_json::<Manifest>(MANIFEST_URL) {
        Ok(m) => {
            let _ = fs::create_dir_all(paths::meta_dir());
            let _ = fs::write(&cache, serde_json::to_vec(&m)?);
            m
        }
        Err(online_err) => {
            let bytes = fs::read(&cache).map_err(|_| online_err)?;
            serde_json::from_slice(&bytes).context("reading cached version list")?
        }
    };
    let mut list: Vec<_> = manifest
        .versions
        .into_iter()
        .filter(|v| v.release_time.as_str() >= OLDEST_SUPPORTED)
        .filter(|v| v.kind == "release" || v.kind == "snapshot")
        .collect();
    list.sort_by(|a, b| b.release_time.cmp(&a.release_time));
    Ok(list)
}

// ---------------------------------------------------------------------------
// Version JSON

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    pub main_class: String,
    pub minecraft_arguments: Option<String>,
    pub arguments: Option<Arguments>,
    pub asset_index: AssetIndexRef,
    pub downloads: VersionDownloads,
    #[serde(default)]
    pub libraries: Vec<Library>,
    pub java_version: Option<JavaVersion>,
    pub logging: Option<Logging>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: OneOrMany },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn values(&self) -> &[String] {
        match self {
            OneOrMany::One(s) => std::slice::from_ref(s),
            OneOrMany::Many(v) => v,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VersionDownloads {
    pub client: Artifact,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Artifact {
    pub path: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
    /// Maven repository base URL. Mod loaders (Fabric) describe libraries
    /// this way instead of with a `downloads` block.
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Library {
    pub fn allowed(&self) -> bool {
        self.rules.as_deref().is_none_or(rules_allow)
    }

    /// The classpath artifact, if this library has one.
    pub fn artifact(&self) -> Option<Artifact> {
        if let Some(a) = self.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            return Some(Artifact { path: a.path.clone().or_else(|| maven_path(&self.name)), ..a.clone() });
        }
        // Maven-style entry: repository base + standard path.
        let base = self.url.as_deref()?;
        let path = maven_path(&self.name)?;
        Some(Artifact {
            url: format!("{}/{path}", base.trim_end_matches('/')),
            path: Some(path),
            sha1: self.sha1.clone(),
            size: self.size,
        })
    }

    /// `group:artifact[:classifier]`, i.e. the name without its version.
    /// Two entries with the same key are the same library.
    pub fn key(&self) -> String {
        let parts: Vec<&str> = self.name.split(':').collect();
        match parts.as_slice() {
            [group, artifact, _version, classifier, ..] => format!("{group}:{artifact}:{classifier}"),
            [group, artifact, ..] => format!("{group}:{artifact}"),
            _ => self.name.clone(),
        }
    }

    /// The old-style (LWJGL 2 era) natives jar for this platform, if any.
    pub fn native_artifact(&self) -> Option<Artifact> {
        let key = self.natives.as_ref()?.get(os_name())?.replace("${arch}", arch_bits());
        let a = self.downloads.as_ref()?.classifiers.as_ref()?.get(&key)?.clone();
        Some(Artifact { path: a.path.clone().or_else(|| maven_path(&format!("{}:{key}", self.name))), ..a })
    }
}

/// `group:artifact:version[:classifier]` -> `group/path/artifact/version/artifact-version[-classifier].jar`
pub fn maven_path(name: &str) -> Option<String> {
    let mut parts = name.split(':');
    let group = parts.next()?;
    let artifact = parts.next()?;
    let version = parts.next()?;
    let classifier = parts.next();
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    Some(format!("{}/{artifact}/{version}/{file}", group.replace('.', "/")))
}

// ---------------------------------------------------------------------------
// Rules

#[derive(Clone, Debug, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn arch_bits() -> &'static str {
    if cfg!(target_pointer_width = "64") { "64" } else { "32" }
}

fn os_version() -> &'static str {
    // Rules only ever test for Windows 10 ("^10\\."), which Windows 11 also reports.
    if cfg!(target_os = "windows") { "10.0" } else { "" }
}

/// Mojang only uses simple anchored prefixes such as `^10\\.`.
fn version_matches(pattern: &str, version: &str) -> bool {
    let prefix = pattern.trim_start_matches('^').replace("\\.", ".");
    version.starts_with(&prefix)
}

fn rule_matches(rule: &Rule) -> bool {
    if let Some(os) = &rule.os {
        if os.name.as_deref().is_some_and(|n| n != os_name()) {
            return false;
        }
        if let Some(arch) = os.arch.as_deref() {
            let ours = if cfg!(target_pointer_width = "64") { "x86_64" } else { "x86" };
            let ours_alt = if cfg!(target_arch = "aarch64") { "arm64" } else { ours };
            if arch != ours && arch != ours_alt {
                return false;
            }
        }
        if os.version.as_deref().is_some_and(|v| !version_matches(v, os_version())) {
            return false;
        }
    }
    // Exile enables none of the optional features (demo mode, custom
    // resolution, quick play), so any rule that requires one does not match.
    if let Some(features) = &rule.features
        && features.values().any(|&wanted| wanted)
    {
        return false;
    }
    true
}

pub fn rules_allow(rules: &[Rule]) -> bool {
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule) {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

pub fn load_version_json(id: &str) -> Result<VersionJson> {
    let path = paths::version_json(id);
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

// ---------------------------------------------------------------------------
// Asset index

#[derive(Clone, Debug, Deserialize)]
pub struct AssetIndex {
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    #[serde(default)]
    pub map_to_resources: bool,
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
    }

    #[test]
    fn os_rules() {
        let only_osx = rule(r#"[{"action":"allow","os":{"name":"osx"}}]"#);
        let not_osx = rule(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#);
        assert_eq!(rules_allow(&only_osx), os_name() == "osx");
        assert_eq!(rules_allow(&not_osx), os_name() != "osx");
    }

    #[test]
    fn feature_rules_never_match() {
        let demo = rule(r#"[{"action":"allow","features":{"is_demo_user":true}}]"#);
        assert!(!rules_allow(&demo));
    }

    #[test]
    fn version_prefix() {
        assert!(version_matches("^10\\.", "10.0"));
        assert!(!version_matches("^10\\.", "6.3"));
    }

    #[test]
    fn parses_mixed_arguments() {
        let args: Arguments = serde_json::from_str(
            r#"{"game":["--username","${auth_player_name}",
                {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],
                 "value":["--width","${resolution_width}"]}],
               "jvm":[{"rules":[{"action":"allow","os":{"name":"windows"}}],"value":"-Dx=y"}]}"#,
        )
        .unwrap();
        assert_eq!(args.game.len(), 3);
        assert!(matches!(&args.jvm[0], Argument::Conditional { value: OneOrMany::One(_), .. }));
    }
}

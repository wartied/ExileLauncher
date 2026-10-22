//! On-disk layout. Everything lives under one root so the launcher is easy to
//! back up or remove:
//!
//! ```text
//! <root>/
//!   settings.toml
//!   account.dat              encrypted session (DPAPI on Windows)
//!   meta/                    cached version manifest
//!   versions/<id>/           <id>.json, <id>.jar, natives/
//!   libraries/               shared maven-style library store
//!   assets/                  indexes/, objects/, virtual/, log_configs/
//!   runtimes/<component>/    Mojang-provided Java runtimes
//!   instances/<id>/          instance.toml, minecraft/ (game dir), logs/
//! ```

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ROOT: OnceLock<PathBuf> = OnceLock::new();

pub fn root() -> &'static Path {
    ROOT.get_or_init(|| {
        // Portable mode / testing: keep everything in a chosen folder.
        if let Some(home) = std::env::var_os("EXILE_HOME").filter(|h| !h.is_empty()) {
            return PathBuf::from(home);
        }
        let base = directories::BaseDirs::new()
            .map(|b| b.data_dir().to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        base.join("Exile")
    })
}

pub fn settings_file() -> PathBuf {
    root().join("settings.toml")
}

pub fn account_file() -> PathBuf {
    root().join("account.dat")
}

pub fn meta_dir() -> PathBuf {
    root().join("meta")
}

pub fn versions_dir() -> PathBuf {
    root().join("versions")
}

pub fn version_dir(id: &str) -> PathBuf {
    versions_dir().join(id)
}

pub fn version_json(id: &str) -> PathBuf {
    version_dir(id).join(format!("{id}.json"))
}

pub fn version_jar(id: &str) -> PathBuf {
    version_dir(id).join(format!("{id}.jar"))
}

pub fn libraries_dir() -> PathBuf {
    root().join("libraries")
}

pub fn assets_dir() -> PathBuf {
    root().join("assets")
}

pub fn runtimes_dir() -> PathBuf {
    root().join("runtimes")
}

pub fn instances_dir() -> PathBuf {
    root().join("instances")
}

/// Joins a relative path that came from remote metadata, refusing anything
/// that could escape `base` (absolute paths, `..`, drive prefixes).
pub fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut out = base.to_path_buf();
    for part in rel.split(['/', '\\']) {
        match part {
            "" | "." => continue,
            ".." => return None,
            p if p.contains(':') => return None,
            p => out.push(p),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_rejects_escapes() {
        let base = Path::new("base");
        assert!(safe_join(base, "../x").is_none());
        assert!(safe_join(base, "a/../../x").is_none());
        assert!(safe_join(base, "C:/Windows").is_none());
        assert_eq!(safe_join(base, "a/b.jar").unwrap(), base.join("a").join("b.jar"));
    }
}

//! Bundles the Exile Client jar into the launcher when it has been built.
//!
//! Build the client first (`client/gradlew build`); this script then embeds
//! `client/build/libs/exile-client-<version>.jar` and sets `cfg(exile_client)`.
//! Without it the launcher still builds, just without the client.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo::rustc-check-cfg=cfg(exile_client)");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let client = manifest.join("../../client");
    let libs = client.join("build/libs");
    println!("cargo::rerun-if-changed={}", libs.display());
    println!("cargo::rerun-if-changed={}", client.join("gradle.properties").display());

    let Some(jar) = find_jar(&libs) else {
        println!("cargo::warning=Exile Client jar not found in client/build/libs; building the launcher without it");
        return;
    };
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("exile-client.jar");
    fs::copy(&jar, &out).expect("copying the Exile Client jar");
    println!("cargo::rerun-if-changed={}", jar.display());

    let props = fs::read_to_string(client.join("gradle.properties")).unwrap_or_default();
    let prop = |key: &str| {
        props
            .lines()
            .find_map(|l| l.strip_prefix(key)?.trim_start().strip_prefix('=').map(|v| v.trim().to_owned()))
            .unwrap_or_default()
    };
    println!("cargo::rustc-cfg=exile_client");
    println!("cargo::rustc-env=EXILE_CLIENT_VERSION={}", prop("mod_version"));
    println!("cargo::rustc-env=EXILE_CLIENT_MINECRAFT={}", prop("minecraft_version"));
}

/// The main jar, not the sources jar.
fn find_jar(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with("exile-client-") && name.ends_with(".jar") && !name.ends_with("-sources.jar")
        })
        .max_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
}

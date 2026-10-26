//! Gives the installer Exile's version (from the launcher's Cargo.toml, so
//! there's only one number to bump) and, on Windows, its icon, file details
//! and a manifest.
//!
//! The manifest matters: without one, Windows treats any exe with "setup" in
//! its name as needing an administrator and asks for one. Exile installs per
//! user, so the manifest says to run with the user's own rights.

use std::env;
use std::fs;
use std::path::PathBuf;

#[cfg(windows)]
#[path = "../ui/src/icon.rs"]
mod icon;
#[cfg(windows)]
#[path = "../ui/build/resources.rs"]
mod resources;

#[cfg(windows)]
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
"#;

fn main() {
    let launcher = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../launcher/Cargo.toml");
    println!("cargo::rerun-if-changed={}", launcher.display());
    let toml = fs::read_to_string(&launcher).expect("reading the launcher's Cargo.toml");
    let version = toml
        .lines()
        .find_map(|l| l.strip_prefix("version = \"")?.strip_suffix('"'))
        .expect("the launcher's version");
    println!("cargo::rustc-env=EXILE_VERSION={version}");

    #[cfg(windows)]
    {
        println!("cargo::rerun-if-changed=../ui/src/icon.rs");
        let out = PathBuf::from(env::var("OUT_DIR").unwrap());
        let details = resources::Details { description: "Exile Setup", version, manifest: Some(MANIFEST) };
        resources::embed(&out, details, true);
    }
}

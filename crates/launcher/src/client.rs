//! The Exile Client, bundled into the launcher at build time (see build.rs).
//!
//! An instance with the client enabled gets Fabric, the client jar (written
//! fresh before every launch, so it always matches the launcher) and a few
//! performance mods it is designed to run alongside.

use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result};
use sha1::{Digest, Sha1};

/// Name of the client jar inside an instance's mods folder.
pub const FILE_NAME: &str = "exile-client.jar";

/// Modrinth projects installed with the client the first time it's enabled.
pub const COMPANIONS: &[(&str, &str)] = &[("fabric-api", "Fabric API"), ("sodium", "Sodium"), ("lithium", "Lithium")];

#[cfg(exile_client)]
const JAR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/exile-client.jar"));

/// Minecraft version this build of the client targets, if one is bundled.
pub fn minecraft_version() -> Option<&'static str> {
    #[cfg(exile_client)]
    return Some(env!("EXILE_CLIENT_MINECRAFT"));
    #[cfg(not(exile_client))]
    None
}

pub fn version() -> Option<&'static str> {
    #[cfg(exile_client)]
    return Some(env!("EXILE_CLIENT_VERSION"));
    #[cfg(not(exile_client))]
    None
}

pub fn supports(game: &str) -> bool {
    minecraft_version() == Some(game)
}

/// Writes the bundled jar into `mods_dir` unless an identical copy is there.
pub fn install(mods_dir: &Path) -> Result<()> {
    #[cfg(exile_client)]
    {
        let path = mods_dir.join(FILE_NAME);
        let same = fs::read(&path).is_ok_and(|existing| existing.len() == JAR.len() && Sha1::digest(&existing) == Sha1::digest(JAR));
        if !same {
            fs::create_dir_all(mods_dir)?;
            let tmp = path.with_extension("jar.tmp");
            fs::write(&tmp, JAR)?;
            fs::rename(&tmp, &path).context("installing Exile Client")?;
        }
        Ok(())
    }
    #[cfg(not(exile_client))]
    {
        let _ = mods_dir;
        anyhow::bail!("This launcher was built without Exile Client.")
    }
}

/// Removes the client jar (client turned off for the instance).
pub fn remove(mods_dir: &Path) {
    let _ = fs::remove_file(mods_dir.join(FILE_NAME));
    let _ = fs::remove_file(mods_dir.join(format!("{FILE_NAME}.disabled")));
}

#[cfg(all(test, exile_client))]
mod tests {
    use super::*;

    #[test]
    fn bundled_jar_is_a_fabric_mod() {
        // A jar is a zip; the client's fabric.mod.json must be inside.
        let zip = zip::ZipArchive::new(std::io::Cursor::new(JAR)).unwrap();
        assert!(zip.file_names().any(|n| n.is_ok_and(|n| n == "fabric.mod.json")));
        assert!(minecraft_version().is_some_and(|v| !v.is_empty()));
    }
}

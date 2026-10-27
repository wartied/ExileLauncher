//! The Minecraft jar Exile launches with.
//!
//! Mojang signs the client jar, and a signed jar's manifest lists a digest
//! for every file in it: 139,581 lines (5 MB) for 26.3. Java 25 takes 50 to
//! 110 seconds to read that manifest whenever it records or builds the
//! startup cache (see `aot`), before the game even begins; with a copy that
//! has no signature, recording starts in 0.4 s (measured with the game's own
//! command line). Vanilla also checks every class it loads against the
//! digests. So games run from an unsigned copy, made once per version. The
//! downloaded jar stays exactly as it is, so it can still be verified
//! against Mojang's checksum.

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read as _, Write as _};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use zip::ZipArchive;
use zip::write::{SimpleFileOptions, ZipWriter};

use crate::paths;

const MANIFEST: &str = "META-INF/MANIFEST.MF";

fn unsigned_path(id: &str) -> PathBuf {
    paths::version_dir(id).join(format!("{id}-unsigned.jar"))
}

/// The jar to put on the classpath: the unsigned copy when there is an
/// up-to-date one, otherwise the downloaded jar.
pub fn for_launch(id: &str) -> PathBuf {
    let copy = unsigned_path(id);
    if is_current(&copy, &paths::version_jar(id)) { copy } else { paths::version_jar(id) }
}

/// Makes the unsigned copy if the jar is signed and the copy is missing or
/// older than the jar (a re-download replaces the jar).
pub fn prepare(id: &str) -> Result<()> {
    let jar = paths::version_jar(id);
    let copy = unsigned_path(id);
    if is_current(&copy, &jar) {
        return Ok(());
    }
    if !strip_signature(&jar, &copy)? {
        let _ = fs::remove_file(&copy);
    }
    Ok(())
}

fn is_current(copy: &Path, jar: &Path) -> bool {
    let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified());
    matches!((modified(copy), modified(jar)), (Ok(c), Ok(j)) if c >= j)
}

/// Writes `jar` to `out` without its signature: the signature files go, and
/// the manifest keeps only its main section (Main-Class and the like).
/// Everything else is copied byte for byte, still compressed. Returns false,
/// writing nothing, when the jar isn't signed.
fn strip_signature(jar: &Path, out: &Path) -> Result<bool> {
    let mut zip = ZipArchive::new(BufReader::new(File::open(jar).with_context(|| format!("opening {}", jar.display()))?))?;
    if !zip.file_names().any(|n| n.is_ok_and(|n| is_signature(&n))) {
        return Ok(false);
    }
    let main = match zip.by_name(MANIFEST) {
        Ok(mut f) => {
            let mut text = String::new();
            f.read_to_string(&mut text)?;
            main_section(&text)
        }
        Err(_) => "Manifest-Version: 1.0\r\n".to_owned(),
    };

    let tmp = out.with_extension("jar.tmp");
    let mut writer = ZipWriter::new(BufWriter::new(File::create(&tmp)?));
    // The manifest goes first, where jar readers expect it.
    writer.start_file(MANIFEST, SimpleFileOptions::default())?;
    writer.write_all(main.as_bytes())?;
    writer.write_all(b"\r\n")?;
    for i in 0..zip.len() {
        let file = zip.by_index_raw(i)?;
        let name = file.name()?.into_owned();
        if name.eq_ignore_ascii_case(MANIFEST) || is_signature(&name) {
            continue;
        }
        writer.raw_copy_file(file)?;
    }
    writer.finish()?.flush()?;
    fs::rename(&tmp, out).with_context(|| format!("writing {}", out.display()))?;
    Ok(true)
}

/// Signature files sit directly in META-INF: NAME.SF with NAME.RSA, .DSA or
/// .EC, and SIG-* files.
fn is_signature(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    let Some(file) = upper.strip_prefix("META-INF/") else { return false };
    !file.contains('/') && (file.ends_with(".SF") || file.ends_with(".RSA") || file.ends_with(".DSA") || file.ends_with(".EC") || file.starts_with("SIG-"))
}

/// The manifest's main attributes: everything before the first blank line.
fn main_section(manifest: &str) -> String {
    let unix = manifest.replace("\r\n", "\n");
    unix.split("\n\n").next().unwrap_or("").lines().map(|l| format!("{l}\r\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_files_are_recognised() {
        assert!(is_signature("META-INF/MOJANGCS.SF"));
        assert!(is_signature("META-INF/mojangcs.rsa"));
        assert!(is_signature("META-INF/SIG-ABC"));
        assert!(!is_signature("META-INF/MANIFEST.MF"));
        assert!(!is_signature("META-INF/LICENSE"));
        assert!(!is_signature("META-INF/versions/9/X.SF"));
        assert!(!is_signature("net/minecraft/A.SF"));
    }

    #[test]
    fn main_section_drops_the_digests() {
        let manifest = "Manifest-Version: 1.0\r\nMain-Class: net.minecraft.client.Main\r\n\r\nName: a.class\r\nSHA-384-Digest: x\r\n y\r\n\r\n";
        assert_eq!(main_section(manifest), "Manifest-Version: 1.0\r\nMain-Class: net.minecraft.client.Main\r\n");
    }

    fn write_jar(path: &Path, entries: &[(&str, &str)]) {
        let mut w = ZipWriter::new(File::create(path).unwrap());
        for (name, body) in entries {
            w.start_file(*name, SimpleFileOptions::default()).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }

    fn names(path: &Path) -> Vec<String> {
        let mut zip = ZipArchive::new(File::open(path).unwrap()).unwrap();
        (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned()).collect()
    }

    #[test]
    fn signed_jar_is_copied_without_its_signature() {
        let dir = std::env::temp_dir().join(format!("exile-jar-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (jar, out) = (dir.join("game.jar"), dir.join("game-unsigned.jar"));

        let manifest = "Manifest-Version: 1.0\r\nMain-Class: M\r\n\r\nName: a/A.class\r\nSHA-384-Digest: x\r\n\r\n";
        write_jar(&jar, &[(MANIFEST, manifest), ("META-INF/MOJANGCS.SF", "sig"), ("META-INF/MOJANGCS.RSA", "sig"), ("a/A.class", "classbytes")]);
        assert!(strip_signature(&jar, &out).unwrap());
        assert_eq!(names(&out), [MANIFEST, "a/A.class"]);
        let mut zip = ZipArchive::new(File::open(&out).unwrap()).unwrap();
        let mut text = String::new();
        zip.by_name(MANIFEST).unwrap().read_to_string(&mut text).unwrap();
        assert_eq!(text, "Manifest-Version: 1.0\r\nMain-Class: M\r\n\r\n");
        let mut class = String::new();
        zip.by_name("a/A.class").unwrap().read_to_string(&mut class).unwrap();
        assert_eq!(class, "classbytes");

        // An unsigned jar is left alone.
        let plain = dir.join("plain.jar");
        write_jar(&plain, &[(MANIFEST, "Manifest-Version: 1.0\r\n"), ("a/A.class", "x")]);
        assert!(!strip_signature(&plain, &dir.join("plain-unsigned.jar")).unwrap());
        assert!(!dir.join("plain-unsigned.jar").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}

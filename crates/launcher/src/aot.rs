//! Java's ahead-of-time class cache (Java 25: JEP 483, 514, 515).
//!
//! The JVM can save the classes a program loads, already parsed and linked,
//! plus method profiles, and map them straight back in on the next start.
//! Measured with the launcher's own command line (26.3, Fabric, Exile Client,
//! Sodium, Lithium; interleaved pairs, every pair agreeing): the menu appears
//! about 8.5% sooner, 9.13 s -> 8.37 s on one day and 12.8 s -> 11.7 s on a
//! slower one. Recording doesn't cost frame rate (Exile Bench, 3 + 3 runs).
//!
//! Per instance it takes three steps, all automatic:
//!
//! 1. The first launch records which classes the game uses
//!    (`-XX:AOTMode=record`); the JVM writes a small file when the game exits.
//! 2. After that game exits, the launcher builds the cache from the recording
//!    in the background (`-XX:AOTMode=create`), without starting the game.
//! 3. Later launches use the cache (`-XX:AOTCache`).
//!
//! The cache is tied to the exact Java and command line. Anything that changes
//! them (version, loader, memory, Java update) starts over at step 1. A cache
//! the JVM can't use for any other reason is ignored by the JVM itself, so the
//! worst case is a normal start.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use anyhow::{Context as _, Result};
use sha1::{Digest, Sha1};

use crate::instance::Instance;
use crate::net;

/// Heap for the background cache build. It only loads classes, it doesn't
/// run the game (measured: 55 s, 529 MB resident in total).
pub const CREATE_HEAP_MB: u64 = 512;

const KEY: &str = "key";
const RECORDING: &str = "game.aotconf";
const CACHE: &str = "game.aot";
const FAILED: &str = "failed";
/// Recording and building list every class they can't store (hundreds, all
/// harmless); keep only real errors in the instance log.
const QUIET: &str = "-Xlog:aot=error,cds=error";

pub fn dir(inst: &Instance) -> PathBuf {
    inst.dir().join("aot")
}

/// Identifies one Java + command line. `jvm_args` is everything before the
/// main class (the game arguments carry the session token, so they're left out).
fn key(java: &Path, jvm_args: &[String]) -> String {
    let mut h = Sha1::new();
    h.update(java.to_string_lossy().as_bytes());
    // A Java update replaces the files in place.
    if let Ok(meta) = fs::metadata(java) {
        h.update(format!("{:?}{}", meta.modified().ok(), meta.len()).as_bytes());
    }
    for arg in jvm_args {
        h.update([0]);
        h.update(arg.as_bytes());
    }
    net::hex(&h.finalize())
}

/// Extra JVM flags for this launch. Never fails: any problem just means no
/// cache this time.
pub fn launch_flags(inst: &Instance, java: &Path, java_major: u32, jvm_args: &[String]) -> Vec<String> {
    if java_major < 25 {
        return Vec::new();
    }
    let dir = dir(inst);
    let key = key(java, jvm_args);
    let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
    if fs::read_to_string(dir.join(KEY)).is_ok_and(|k| k == key) {
        adopt_finished_build(&dir);
        if dir.join(CACHE).is_file() {
            return vec![format!("-XX:AOTCache={}", path(CACHE))];
        }
        if dir.join(FAILED).exists() || dir.join(RECORDING).is_file() {
            // Gave up on this setup, or the build is still to come.
            return Vec::new();
        }
    } else {
        // New setup: clear out the old one. A cache still mapped by a running
        // game can't be deleted on Windows; try again next launch.
        if dir.exists() && fs::remove_dir_all(&dir).is_err() {
            return Vec::new();
        }
        if fs::create_dir_all(&dir).and_then(|_| fs::write(dir.join(KEY), &key)).is_err() {
            return Vec::new();
        }
    }
    vec!["-XX:AOTMode=record".into(), format!("-XX:AOTConfiguration={}", path(RECORDING)), QUIET.into()]
}

/// True right after `launch_flags` chose to record: the recording itself is
/// only written when that game exits.
pub fn recording(inst: &Instance) -> bool {
    let dir = dir(inst);
    dir.join(KEY).is_file() && ![RECORDING, CACHE, FAILED].iter().any(|f| dir.join(f).exists())
}

/// True when a recording is waiting to be turned into a cache.
pub fn pending(inst: &Instance) -> bool {
    let dir = dir(inst);
    dir.join(RECORDING).is_file() && !dir.join(CACHE).exists() && !dir.join(FAILED).exists()
}

/// Starts the background build. `jvm_args` must be the game's own JVM
/// arguments (same classpath and flags), without the AOT flags.
pub fn start_create(inst: &Instance, java: &Path, jvm_args: &[String]) -> Result<Child> {
    let dir = dir(inst);
    let mut args: Vec<String> = jvm_args.iter().filter(|a| !a.starts_with("-Xms") && !a.starts_with("-Xmx")).cloned().collect();
    args.push(format!("-Xmx{CREATE_HEAP_MB}M"));
    args.push("-XX:AOTMode=create".into());
    args.push(format!("-XX:AOTConfiguration={}", dir.join(RECORDING).display()));
    args.push(format!("-XX:AOTCache={}", dir.join(format!("{CACHE}.tmp")).display()));
    args.push(QUIET.into());
    let log = fs::File::create(dir.join("create.log"))?;
    let mut cmd = Command::new(java);
    cmd.args(&args)
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // Stays out of the way of anything the user is doing.
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        cmd.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    cmd.spawn().with_context(|| format!("starting {}", java.display()))
}

/// A build keeps running if the launcher closes, and then nothing moves its
/// result into place. The JVM prints this line only once the file is whole.
fn adopt_finished_build(dir: &Path) {
    let tmp = dir.join(format!("{CACHE}.tmp"));
    if !dir.join(CACHE).exists()
        && tmp.is_file()
        && fs::read_to_string(dir.join("create.log")).is_ok_and(|log| log.contains("AOTCache creation is complete"))
    {
        finish_create(dir, true);
    }
}

/// Called when the background build exits.
pub fn finish_create(dir: &Path, ok: bool) {
    let tmp = dir.join(format!("{CACHE}.tmp"));
    if ok && tmp.is_file() && fs::rename(&tmp, dir.join(CACHE)).is_ok() {
        let _ = fs::remove_file(dir.join(RECORDING));
    } else {
        // Don't record again on every launch for a setup that can't be cached.
        let _ = fs::remove_file(&tmp);
        let _ = fs::write(dir.join(FAILED), "");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finished_build_is_adopted_once_complete() {
        let dir = std::env::temp_dir().join(format!("exile-aot-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(RECORDING), "rec").unwrap();
        fs::write(dir.join(format!("{CACHE}.tmp")), "cache").unwrap();

        // Still being written: left alone.
        fs::write(dir.join("create.log"), "Reading AOTConfiguration ...").unwrap();
        adopt_finished_build(&dir);
        assert!(!dir.join(CACHE).exists());

        fs::write(dir.join("create.log"), "Reading ...\nAOTCache creation is complete: game.aot.tmp 5 bytes").unwrap();
        adopt_finished_build(&dir);
        assert_eq!(fs::read_to_string(dir.join(CACHE)).unwrap(), "cache");
        assert!(!dir.join(RECORDING).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn key_follows_the_command_line() {
        let java = Path::new("no-such-java");
        let a = key(java, &["-Xmx4096M".into(), "-cp".into(), "a.jar".into()]);
        assert_eq!(a, key(java, &["-Xmx4096M".into(), "-cp".into(), "a.jar".into()]));
        assert_ne!(a, key(java, &["-Xmx2048M".into(), "-cp".into(), "a.jar".into()]));
        // Argument boundaries count: ["ab"] is not ["a", "b"].
        assert_ne!(key(java, &["ab".into()]), key(java, &["a".into(), "b".into()]));
    }
}

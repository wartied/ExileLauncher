//! Launcher updates. The Exile Client is bundled in the launcher, so updating
//! the launcher updates the client too (it's written fresh at every launch).
//!
//! Every release publishes `update.json` next to its files, and exiles.dev
//! points at the latest one. For each platform it lists the download, its
//! size and SHA-256, and an Ed25519 signature over
//!
//! ```text
//! exile-update\n<version>\n<platform>\n<sha256>\n
//! ```
//!
//! made with Exile's release key in CI. The launcher checks that signature
//! with the public key built in below before it downloads anything, and the
//! file's hash afterwards. Whoever controls the website or the download host
//! can't make the launcher run anything Exile didn't sign; the version in the
//! signed text also stops an old signed release being served as "new".
//!
//! Installing: on Windows a running .exe can be renamed but not overwritten,
//! so the current file moves aside to `exile.old` (deleted on the next start)
//! and the new one takes its name. On Linux the new file simply replaces the
//! old path; the running process keeps its copy until it exits.

use std::fs::{self, File};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use anyhow::{Context as _, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::net::{self, Progress};

/// Where the launcher looks for updates (exiles.dev redirects it to the
/// latest release's `update.json`). `EXILE_UPDATE_URL` overrides it for
/// testing; updates must still be signed with the release key.
const MANIFEST_URL: &str = "https://exiles.dev/update.json";

fn manifest_url() -> String {
    std::env::var("EXILE_UPDATE_URL").ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| MANIFEST_URL.to_owned())
}

/// Public half of Exile's release signing key (Ed25519).
const PUBLIC_KEY: [u8; 32] = [
    251, 99, 168, 189, 211, 231, 66, 240, 18, 87, 45, 109, 4, 196, 254, 30, 80, 30, 248, 207, 126, 123, 190, 130, 71, 48, 3, 220,
    104, 89, 204, 47,
];

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

pub const PLATFORM: &str = if cfg!(windows) { "windows-x86_64" } else { "linux-x86_64" };

/// No release is anywhere near this; anything bigger is refused.
const MAX_SIZE: u64 = 256 * 1024 * 1024;

#[derive(Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    notes: String,
    platforms: std::collections::HashMap<String, Asset>,
}

#[derive(Clone, Debug, Deserialize)]
struct Asset {
    url: String,
    sha256: String,
    size: u64,
    signature: String,
}

/// A newer, correctly signed release for this platform.
#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    asset: Asset,
}

/// Checks for a newer release. `Ok(None)` means up to date (or no build for
/// this platform yet).
pub fn check() -> Result<Option<Release>> {
    let manifest: Manifest = net::get_json(&manifest_url()).context("checking for updates")?;
    if !is_newer(&manifest.version, CURRENT) {
        return Ok(None);
    }
    let Some(asset) = manifest.platforms.get(PLATFORM).cloned() else { return Ok(None) };
    verify(&manifest.version, PLATFORM, &asset.sha256, &asset.signature, &PUBLIC_KEY)?;
    if asset.size == 0 || asset.size > MAX_SIZE {
        bail!("update size {} is not plausible", asset.size);
    }
    Ok(Some(Release { version: manifest.version, notes: manifest.notes, asset }))
}

fn statement(version: &str, platform: &str, sha256: &str) -> String {
    format!("exile-update\n{version}\n{platform}\n{}\n", sha256.to_ascii_lowercase())
}

fn verify(version: &str, platform: &str, sha256: &str, signature: &str, key: &[u8; 32]) -> Result<()> {
    let key = VerifyingKey::from_bytes(key).map_err(|_| anyhow!("bad update key"))?;
    let sig: [u8; 64] = STANDARD
        .decode(signature.trim())
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| anyhow!("update signature is malformed"))?;
    key.verify_strict(statement(version, platform, sha256).as_bytes(), &Signature::from_bytes(&sig))
        .map_err(|_| anyhow!("update signature doesn't match Exile's release key; not installing it"))
}

/// `a` is a later version than `b` ("major.minor.patch"; anything after a
/// '-' is ignored).
fn is_newer(a: &str, b: &str) -> bool {
    fn parse(v: &str) -> Option<(u64, u64, u64)> {
        let mut it = v.trim().trim_start_matches('v').split('-').next()?.split('.').map(|p| p.parse::<u64>().ok());
        Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
    }
    matches!((parse(a), parse(b)), (Some(a), Some(b)) if a > b)
}

fn exe() -> Result<PathBuf> {
    std::env::current_exe().context("finding the launcher's own file")
}

fn staged_path(exe: &Path) -> PathBuf {
    exe.with_file_name(if cfg!(windows) { "exile.update.exe" } else { ".exile.update" })
}

fn old_path(exe: &Path) -> PathBuf {
    exe.with_file_name(if cfg!(windows) { "exile.old" } else { ".exile.old" })
}

/// Downloads the release next to the launcher and checks it. Returns the
/// staged file, ready for [`install`].
pub fn download(release: &Release, progress: &Progress) -> Result<PathBuf> {
    let exe = exe()?;
    let staged = staged_path(&exe);
    progress.total_bytes.store(release.asset.size, Ordering::Relaxed);
    progress.done_bytes.store(0, Ordering::Relaxed);

    let result = (|| -> Result<()> {
        let mut out = File::create(&staged).with_context(|| {
            format!(
                "Exile can't update itself in {}. Download the new version from exiles.dev instead.",
                exe.parent().map_or_else(String::new, |p| p.display().to_string())
            )
        })?;
        let mut resp = net::agent().get(&release.asset.url).call().context("downloading the update")?;
        if !resp.status().is_success() {
            bail!("downloading the update: HTTP {}", resp.status().as_u16());
        }
        let mut reader = resp.body_mut().as_reader();
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut written = 0u64;
        loop {
            if progress.cancelled() {
                bail!(net::Cancelled);
            }
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > release.asset.size {
                bail!("the update is larger than announced");
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n])?;
            progress.done_bytes.store(written, Ordering::Relaxed);
        }
        out.flush()?;
        if written != release.asset.size {
            bail!("the update download was incomplete");
        }
        if !net::hex(&hasher.finalize()).eq_ignore_ascii_case(&release.asset.sha256) {
            bail!("the update didn't match its signed checksum; not installing it");
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&staged);
        return Err(e);
    }
    Ok(staged)
}

/// Puts a staged update in place of the running launcher. Takes effect the
/// next time Exile starts (see [`restart`]).
pub fn install(staged: &Path) -> Result<()> {
    let exe = exe()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(staged, fs::Permissions::from_mode(0o755))?;
    }
    if cfg!(windows) {
        let old = old_path(&exe);
        let _ = fs::remove_file(&old);
        fs::rename(&exe, &old).context("moving the old launcher aside")?;
        if let Err(e) = fs::rename(staged, &exe) {
            // Put the working launcher back rather than leave nothing behind.
            let _ = fs::rename(&old, &exe);
            return Err(e).context("putting the new launcher in place");
        }
    } else {
        fs::rename(staged, &exe).context("putting the new launcher in place")?;
    }
    Ok(())
}

/// Starts the (new) launcher and ends this one.
pub fn restart() -> ! {
    if let Ok(exe) = exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    std::process::exit(0);
}

/// Removes what an earlier update left behind. Called at start-up. Right
/// after "Restart" the previous launcher may still be exiting and holding its
/// file, so this keeps trying for a few seconds in the background.
pub fn clean_up() {
    let Ok(exe) = exe() else { return };
    let _ = fs::remove_file(staged_path(&exe));
    let old = old_path(&exe);
    if old.exists() {
        std::thread::spawn(move || {
            for _ in 0..20 {
                if fs::remove_file(&old).is_ok() || !old.exists() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway key and a signature made by `openssl pkeyutl -sign -rawin`,
    /// the same command the release workflow uses.
    const TEST_KEY: [u8; 32] = [
        190, 49, 103, 158, 160, 181, 33, 239, 46, 252, 221, 189, 104, 89, 43, 190, 1, 107, 133, 102, 198, 160, 18, 7, 62, 132, 245,
        205, 127, 22, 86, 83,
    ];
    const TEST_SIG: &str = "hbtv9xOGJuldBXAOTDsaOyagITdfr9Jb7nH9XNqDZTirZyEO9m3TXEQzJA97VCSoVw8ISMkyhs6ruDBH4mD2CA==";

    #[test]
    fn accepts_openssl_signatures() {
        verify("0.2.0", "windows-x86_64", "abc123", TEST_SIG, &TEST_KEY).unwrap();
    }

    #[test]
    fn rejects_anything_else() {
        // Different version, platform or file: the signature no longer matches.
        assert!(verify("0.3.0", "windows-x86_64", "abc123", TEST_SIG, &TEST_KEY).is_err());
        assert!(verify("0.2.0", "linux-x86_64", "abc123", TEST_SIG, &TEST_KEY).is_err());
        assert!(verify("0.2.0", "windows-x86_64", "abc124", TEST_SIG, &TEST_KEY).is_err());
        // Signed by a different key (Exile's real one didn't sign this).
        assert!(verify("0.2.0", "windows-x86_64", "abc123", TEST_SIG, &PUBLIC_KEY).is_err());
        assert!(verify("0.2.0", "windows-x86_64", "abc123", "not base64", &TEST_KEY).is_err());
    }

    #[test]
    fn version_order() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("v0.2.1", "0.2.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }
}

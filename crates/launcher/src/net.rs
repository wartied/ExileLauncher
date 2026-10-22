//! HTTP and verified downloads.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use serde::de::DeserializeOwned;
use sha1::{Digest, Sha1};

const USER_AGENT: &str = concat!("ExileLauncher/", env!("CARGO_PKG_VERSION"));
const PARALLEL_DOWNLOADS: usize = 12;
const ATTEMPTS: u32 = 3;

pub fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .user_agent(USER_AGENT)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            // A stalled transfer fails instead of hanging an install forever.
            .timeout_per_read(Some(Duration::from_secs(30)))
            // Callers inspect status codes themselves so error bodies can be read.
            .http_status_as_error(false)
            .build()
            .new_agent()
    })
}

fn check_status(resp: &ureq::http::Response<ureq::Body>, url: &str) -> Result<()> {
    let status = resp.status();
    if !status.is_success() {
        bail!("{url} returned HTTP {}", status.as_u16());
    }
    Ok(())
}

pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let mut resp = agent().get(url).call().with_context(|| format!("requesting {url}"))?;
    check_status(&resp, url)?;
    let body = resp.body_mut().with_config().limit(64 * 1024 * 1024).read_to_vec()?;
    serde_json::from_slice(&body).with_context(|| format!("parsing {url}"))
}

pub fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let mut resp = agent().get(url).call().with_context(|| format!("requesting {url}"))?;
    check_status(&resp, url)?;
    Ok(resp.body_mut().with_config().limit(64 * 1024 * 1024).read_to_vec()?)
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// Full check: size and SHA-1.
pub fn file_matches(path: &Path, sha1: Option<&str>, size: Option<u64>) -> bool {
    let Ok(meta) = fs::metadata(path) else { return false };
    if !meta.is_file() || size.is_some_and(|s| s != meta.len()) {
        return false;
    }
    match sha1 {
        Some(expected) => sha1_file(path).is_ok_and(|h| h.eq_ignore_ascii_case(expected)),
        None => true,
    }
}

/// Cheap check used before every launch: the file exists with the right size.
pub fn file_present(path: &Path, size: Option<u64>) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && size.is_none_or(|s| s == m.len()))
}

#[derive(Clone, Debug)]
pub struct Download {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// Progress shared between a worker and the UI. Everything is atomic so the UI
/// never blocks on a download thread.
#[derive(Default)]
pub struct Progress {
    pub done_bytes: AtomicU64,
    pub total_bytes: AtomicU64,
    pub cancel: AtomicBool,
    stage: Mutex<String>,
}

impl Progress {
    pub fn set_stage(&self, stage: impl Into<String>) {
        *self.stage.lock().unwrap_or_else(|e| e.into_inner()) = stage.into();
    }

    pub fn stage(&self) -> String {
        self.stage.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn fraction(&self) -> f32 {
        let total = self.total_bytes.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        (self.done_bytes.load(Ordering::Relaxed) as f64 / total as f64).clamp(0.0, 1.0) as f32
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn check_cancel(&self) -> Result<()> {
        if self.cancelled() { Err(anyhow!(Cancelled)) } else { Ok(()) }
    }
}

#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for Cancelled {}

pub fn is_cancelled(err: &anyhow::Error) -> bool {
    err.downcast_ref::<Cancelled>().is_some()
}

/// Streams `dl` to disk, hashing as it goes, then moves it into place.
/// Bytes are credited to `progress` as they arrive.
fn download_once(dl: &Download, progress: &Progress) -> Result<()> {
    if let Some(parent) = dl.path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut resp = agent().get(&dl.url).call().with_context(|| format!("requesting {}", dl.url))?;
    check_status(&resp, &dl.url)?;

    // Unique temp name: two installs of the same version may race on a file.
    let mut nonce = [0u8; 6];
    getrandom::fill(&mut nonce).map_err(|e| anyhow!("rng: {e}"))?;
    let tmp = dl.path.with_extension(format!("part-{}", hex(&nonce)));

    let result = (|| -> Result<()> {
        let mut reader = resp.body_mut().as_reader();
        let mut out = File::create(&tmp)?;
        let mut hasher = Sha1::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut written = 0u64;
        loop {
            progress.check_cancel()?;
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n])?;
            written += n as u64;
            progress.done_bytes.fetch_add(n as u64, Ordering::Relaxed);
        }
        out.flush()?;
        drop(out);
        if let Some(size) = dl.size
            && size != written
        {
            bail!("{}: expected {size} bytes, got {written}", dl.url);
        }
        if let Some(expected) = &dl.sha1 {
            let got = hex(&hasher.finalize());
            if !got.eq_ignore_ascii_case(expected) {
                bail!("{}: checksum mismatch", dl.url);
            }
        }
        if let Err(e) = fs::rename(&tmp, &dl.path) {
            // Someone else may have finished the same file first.
            if !file_matches(&dl.path, dl.sha1.as_deref(), dl.size) {
                return Err(e).with_context(|| format!("saving {}", dl.path.display()));
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn download(dl: &Download, progress: &Progress) -> Result<()> {
    let mut last_err = None;
    for attempt in 0..ATTEMPTS {
        let before = progress.done_bytes.load(Ordering::Relaxed);
        match download_once(dl, progress) {
            Ok(()) => return Ok(()),
            Err(e) if is_cancelled(&e) => return Err(e),
            Err(e) => {
                // Take back the bytes of the failed attempt so progress stays honest.
                let after = progress.done_bytes.load(Ordering::Relaxed);
                progress.done_bytes.fetch_sub(after.saturating_sub(before), Ordering::Relaxed);
                last_err = Some(e);
                std::thread::sleep(Duration::from_millis(500 * (attempt as u64 + 1)));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("download failed")))
}

/// Downloads everything that isn't already on disk and valid, using a small
/// pool of threads. The pool is scoped: all threads have finished when this
/// returns, whether it succeeds, fails or is cancelled.
pub fn download_all(downloads: Vec<Download>, progress: &Progress) -> Result<()> {
    // Verify existing files in parallel as well; there can be thousands.
    let needed: Mutex<Vec<Download>> = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..PARALLEL_DOWNLOADS {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(dl) = downloads.get(i) else { break };
                    if progress.cancelled() {
                        break;
                    }
                    if file_matches(&dl.path, dl.sha1.as_deref(), dl.size) {
                        continue;
                    }
                    needed.lock().unwrap_or_else(|e| e.into_inner()).push(dl.clone());
                }
            });
        }
    });
    progress.check_cancel()?;
    let needed = needed.into_inner().unwrap_or_else(|e| e.into_inner());
    if needed.is_empty() {
        return Ok(());
    }

    progress.set_stage("Downloading");
    progress.total_bytes.fetch_add(needed.iter().filter_map(|d| d.size).sum(), Ordering::Relaxed);
    let next = AtomicUsize::new(0);
    let first_error: Mutex<Option<anyhow::Error>> = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..PARALLEL_DOWNLOADS.min(needed.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(dl) = needed.get(i) else { break };
                    if progress.cancelled() {
                        break;
                    }
                    if let Err(e) = download(dl, progress) {
                        let mut slot = first_error.lock().unwrap_or_else(|e| e.into_inner());
                        if slot.is_none() {
                            *slot = Some(e);
                        }
                        // Stop the other workers; one failure fails the install.
                        progress.cancel.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            });
        }
    });
    if let Some(e) = first_error.into_inner().unwrap_or_else(|e| e.into_inner()) {
        return Err(e);
    }
    progress.check_cancel()
}

/// Minimal `application/x-www-form-urlencoded` / query-string encoding.
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(hi), Some(lo)) => {
                        out.push((hi * 16 + lo) as u8);
                        i += 2;
                    }
                    _ => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_roundtrip() {
        let s = "a b&c=d/é~";
        assert_eq!(url_decode(&url_encode(s)), s);
        assert_eq!(url_decode("M.C5%2B%2Fx"), "M.C5+/x");
        assert_eq!(url_decode("trailing%2"), "trailing%2");
    }
}

//! The launcher inside the installer. The release workflow appends exile.exe
//! to the built installer (`.github/scripts/pack_installer.py`), followed by
//! a fixed-size trailer:
//!
//! ```text
//! [installer][exile.exe][SHA-256 of exile.exe: 32][its length: u64 LE]["EXILEPK1"]
//! ```
//!
//! Windows runs the installer part and ignores what follows it. Copying only
//! the installer part gives the uninstaller, so it doesn't carry a second
//! copy of Exile.

use std::io::{self, Read, Seek, SeekFrom};

use anyhow::{Result, bail};
use sha2::{Digest, Sha256};

pub const MAGIC: &[u8; 8] = b"EXILEPK1";
const TRAILER: u64 = 32 + 8 + 8;
/// Far bigger than the launcher will ever be; a bad length can't make the
/// installer try to read gigabytes.
const MAX_LEN: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Payload {
    /// Where the launcher starts, which is also the installer's own length.
    pub start: u64,
    pub len: u64,
    sha256: [u8; 32],
}

/// Finds the launcher at the end of `file`. `None` for a bare installer
/// (the uninstaller, or a development build).
pub fn find(file: &mut (impl Read + Seek)) -> io::Result<Option<Payload>> {
    let size = file.seek(SeekFrom::End(0))?;
    if size < TRAILER {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(size - TRAILER))?;
    let mut trailer = [0u8; TRAILER as usize];
    file.read_exact(&mut trailer)?;
    if &trailer[40..] != MAGIC {
        return Ok(None);
    }
    let len = u64::from_le_bytes(trailer[32..40].try_into().expect("8 bytes"));
    if len == 0 || len > MAX_LEN || len > size - TRAILER {
        return Ok(None);
    }
    Ok(Some(Payload { start: size - TRAILER - len, len, sha256: trailer[..32].try_into().expect("32 bytes") }))
}

/// Reads the launcher and checks it arrived intact.
pub fn read(file: &mut (impl Read + Seek), payload: &Payload) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(payload.start))?;
    let mut bytes = vec![0u8; payload.len as usize];
    file.read_exact(&mut bytes)?;
    if Sha256::digest(&bytes).as_slice() != payload.sha256 {
        bail!("The installer is damaged. Download it again from exiles.dev.");
    }
    Ok(bytes)
}

/// The installer without the launcher: what becomes uninstall.exe.
pub fn read_installer(file: &mut (impl Read + Seek), payload: &Payload) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = vec![0u8; payload.start as usize];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    /// What pack_installer.py writes.
    fn pack(installer: &[u8], launcher: &[u8]) -> Vec<u8> {
        let mut out = installer.to_vec();
        out.extend_from_slice(launcher);
        out.extend_from_slice(&Sha256::digest(launcher));
        out.extend_from_slice(&(launcher.len() as u64).to_le_bytes());
        out.extend_from_slice(MAGIC);
        out
    }

    #[test]
    fn finds_and_checks_the_launcher() {
        let mut file = Cursor::new(pack(b"MZ installer", b"MZ launcher"));
        let payload = find(&mut file).unwrap().unwrap();
        assert_eq!(payload.start, 12);
        assert_eq!(read(&mut file, &payload).unwrap(), b"MZ launcher");
        assert_eq!(read_installer(&mut file, &payload).unwrap(), b"MZ installer");
    }

    #[test]
    fn bare_installer_has_none() {
        assert_eq!(find(&mut Cursor::new(b"MZ just the installer, long enough to hold a trailer".to_vec())).unwrap(), None);
        assert_eq!(find(&mut Cursor::new(b"MZ".to_vec())).unwrap(), None);
    }

    #[test]
    fn damage_is_caught() {
        let mut bytes = pack(b"MZ installer", b"MZ launcher");
        bytes[14] ^= 1;
        let mut file = Cursor::new(bytes);
        let payload = find(&mut file).unwrap().unwrap();
        assert!(read(&mut file, &payload).is_err());

        // A length that runs past the start of the file.
        let mut bytes = pack(b"", b"MZ launcher");
        let at = bytes.len() - 16;
        bytes[at..at + 8].copy_from_slice(&1000u64.to_le_bytes());
        assert_eq!(find(&mut Cursor::new(bytes)).unwrap(), None);
    }
}

//! The Windows side: where things go, shortcuts, and Exile's entry in
//! Settings > Apps. Everything is per user, so no administrator is needed and
//! the launcher can update itself in place.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
    IPersistFile,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_RoamingAppData, FOLDERID_UserProgramFiles, IShellLinkW, KF_FLAG_CREATE,
    SHGetKnownFolderPath, ShellLink,
};
use windows::core::{GUID, HSTRING, Interface as _};
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Exile";
const LINK: &str = "Exile.lnk";

#[derive(Clone)]
pub struct Places {
    /// %LOCALAPPDATA%\Programs\Exile
    pub dir: PathBuf,
    /// The Start menu's Programs folder.
    start_menu: PathBuf,
    desktop: PathBuf,
    /// Where the launcher keeps instances, worlds and settings (%APPDATA%\Exile).
    pub data: PathBuf,
    /// Exile's entry under HKEY_CURRENT_USER.
    key: String,
}

impl Places {
    pub fn find() -> Result<Self> {
        // Testing: everything goes in this folder instead, and the Apps entry
        // is "Exile (test)", so a real install and its data are never touched.
        if let Some(root) = std::env::var_os("EXILE_SETUP_ROOT").filter(|r| !r.is_empty()).map(PathBuf::from) {
            return Ok(Self {
                dir: root.join("Programs").join("Exile"),
                start_menu: root.join("Start Menu"),
                desktop: root.join("Desktop"),
                data: root.join("AppData").join("Exile"),
                key: format!("{UNINSTALL_KEY} (test)"),
            });
        }
        Ok(Self {
            dir: known_folder(&FOLDERID_UserProgramFiles)?.join("Exile"),
            start_menu: known_folder(&FOLDERID_Programs)?,
            desktop: known_folder(&FOLDERID_Desktop)?,
            data: known_folder(&FOLDERID_RoamingAppData)?.join("Exile"),
            key: UNINSTALL_KEY.to_owned(),
        })
    }

    pub fn exe(&self) -> PathBuf {
        self.dir.join("exile.exe")
    }

    pub fn uninstaller(&self) -> PathBuf {
        self.dir.join("uninstall.exe")
    }

    pub fn start_menu_link(&self) -> PathBuf {
        self.start_menu.join(LINK)
    }

    pub fn desktop_link(&self) -> PathBuf {
        self.desktop.join(LINK)
    }
}

fn known_folder(id: &GUID) -> Result<PathBuf> {
    // SAFETY: `id` is one of the KNOWNFOLDERID constants. The returned string
    // belongs to us: it's copied, then freed exactly once.
    unsafe {
        let raw = SHGetKnownFolderPath(id, KF_FLAG_CREATE, None).context("finding a system folder")?;
        let path = raw.to_string();
        CoTaskMemFree(Some(raw.0 as *const _));
        Ok(PathBuf::from(path.context("reading a system folder's path")?))
    }
}

/// COM for the current thread, for as long as this lives.
struct Com;

impl Com {
    fn init() -> Result<Self> {
        // SAFETY: plain initialisation call; balanced by CoUninitialize in Drop.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok().context("starting COM")?;
        Ok(Com)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: matches the successful CoInitializeEx in `init`.
        unsafe { CoUninitialize() };
    }
}

/// Writes a shortcut at `link` that starts `target`, with `target`'s icon.
pub fn create_shortcut(link: &Path, target: &Path, description: &str) -> Result<()> {
    if let Some(folder) = link.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let _com = Com::init()?;
    let dir = target.parent().unwrap_or(target);
    // SAFETY: COM is initialised on this thread for the whole block, and every
    // string argument outlives the call it's passed to.
    unsafe {
        let shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        shell.SetPath(&HSTRING::from(target.as_os_str()))?;
        shell.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
        shell.SetDescription(&HSTRING::from(description))?;
        shell.SetIconLocation(&HSTRING::from(target.as_os_str()), 0)?;
        shell.cast::<IPersistFile>()?.Save(&HSTRING::from(link.as_os_str()), true)?;
    }
    Ok(())
}

/// Lists Exile in Settings > Apps, where "Uninstall" runs uninstall.exe.
pub fn register(places: &Places, version: &str, size_kb: u32) -> Result<()> {
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(&places.key).context("adding Exile to Apps")?;
    let exe = places.exe().display().to_string();
    let uninstall = format!("\"{}\" --uninstall", places.uninstaller().display());
    let name = if places.key == UNINSTALL_KEY { "Exile" } else { "Exile (test)" };
    key.set_value("DisplayName", &name)?;
    key.set_value("DisplayVersion", &version)?;
    key.set_value("DisplayIcon", &exe)?;
    key.set_value("Publisher", &"Exile")?;
    key.set_value("URLInfoAbout", &"https://exiles.dev")?;
    key.set_value("InstallLocation", &places.dir.display().to_string())?;
    key.set_value("UninstallString", &uninstall)?;
    key.set_value("QuietUninstallString", &format!("{uninstall} --quiet"))?;
    key.set_value("EstimatedSize", &size_kb)?;
    key.set_value("NoModify", &1u32)?;
    key.set_value("NoRepair", &1u32)?;
    Ok(())
}

pub fn unregister(places: &Places) -> Result<()> {
    match RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&places.key) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e).context("removing Exile from Apps"),
        _ => Ok(()),
    }
}

/// The version in Apps, if Exile is installed.
pub fn installed_version(places: &Places) -> Option<String> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey(&places.key).ok()?.get_value("DisplayVersion").ok()
}

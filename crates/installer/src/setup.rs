//! Installing and uninstalling. Each step reports how far along it is, for
//! the progress bar.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow};

use crate::payload::{self, Payload};
use crate::system::{self, Places};

pub const VERSION: &str = env!("EXILE_VERSION");
const DESCRIPTION: &str = "Exile, a fast Minecraft launcher";

/// Files Exile keeps next to exile.exe: the uninstaller, and what updates
/// and this installer leave behind for a moment.
const OTHER_FILES: [&str; 4] = ["uninstall.exe", "exile.old", "exile.update.exe", "exile.setup.tmp"];

pub type Report<'a> = &'a dyn Fn(f32, &str);

/// Installs (or reinstalls) Exile from the launcher packed into `me`.
pub fn install(places: &Places, me: &Path, payload: &Payload, desktop: bool, report: Report) -> Result<()> {
    report(0.05, "Unpacking Exile");
    let mut file = fs::File::open(me).context("opening the installer")?;
    let launcher = payload::read(&mut file, payload)?;
    let uninstaller = payload::read_installer(&mut file, payload)?;

    report(0.3, "Copying Exile");
    fs::create_dir_all(&places.dir).with_context(|| format!("creating {}", places.dir.display()))?;
    let temp = places.dir.join("exile.setup.tmp");
    fs::write(&temp, &launcher).context("copying Exile")?;
    replace(&temp, &places.exe())?;
    fs::write(places.uninstaller(), &uninstaller).context("adding the uninstaller")?;

    report(0.65, "Adding shortcuts");
    system::create_shortcut(&places.start_menu_link(), &places.exe(), DESCRIPTION).context("adding Exile to the Start menu")?;
    if desktop {
        system::create_shortcut(&places.desktop_link(), &places.exe(), DESCRIPTION).context("adding the desktop shortcut")?;
    } else {
        remove(&places.desktop_link())?;
    }

    report(0.85, "Adding Exile to Apps");
    let size_kb = ((launcher.len() + uninstaller.len()) / 1024) as u32;
    system::register(places, VERSION, size_kb)?;
    report(1.0, "Done");
    Ok(())
}

/// Puts `new` in place of `exe`. If Exile is running its file can't be
/// replaced, but it can be renamed: it moves aside to exile.old, which the
/// launcher deletes when it next starts.
fn replace(new: &Path, exe: &Path) -> Result<()> {
    if exe.exists() && fs::remove_file(exe).is_err() {
        let old = exe.with_file_name("exile.old");
        let _ = fs::remove_file(&old);
        fs::rename(exe, &old).context("moving the old Exile aside")?;
    }
    fs::rename(new, exe).context("putting Exile in place")
}

pub fn uninstall(places: &Places, delete_data: bool, report: Report) -> Result<()> {
    report(0.1, "Removing Exile");
    remove(&places.exe()).map_err(|e| {
        if is_in_use(&e) { anyhow!("Exile is still open. Close it, then try again.") } else { e }
    })?;
    for name in OTHER_FILES {
        let path = places.dir.join(name);
        // uninstall.exe may still be closing after starting this copy.
        let mut attempt = 0;
        while let Err(e) = remove(&path) {
            attempt += 1;
            if attempt == 20 {
                return Err(e);
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }
    // Only if nothing else is in there.
    let _ = fs::remove_dir(&places.dir);

    report(0.4, "Removing shortcuts");
    remove(&places.start_menu_link())?;
    remove(&places.desktop_link())?;

    report(0.6, "Removing Exile from Apps");
    system::unregister(places)?;

    if delete_data {
        report(0.7, "Deleting instances, worlds and settings");
        match fs::remove_dir_all(&places.data) {
            Err(e) if e.kind() != ErrorKind::NotFound => {
                return Err(anyhow!(
                    "Exile is uninstalled, but some of its data couldn't be deleted ({e}). If a game is still open, close it and delete {} yourself.",
                    places.data.display()
                ));
            }
            _ => {}
        }
    }
    report(1.0, "Done");
    Ok(())
}

/// Deletes a file if it's there.
fn remove(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(e).with_context(|| format!("removing {}", path.display())),
        _ => Ok(()),
    }
}

/// Access denied or a sharing violation: Windows' answer for a running exe.
fn is_in_use(e: &anyhow::Error) -> bool {
    e.chain()
        .filter_map(|c| c.downcast_ref::<std::io::Error>())
        .any(|io| io.kind() == ErrorKind::PermissionDenied || io.raw_os_error() == Some(32))
}

/// The uninstaller can't delete itself while it runs, so it first copies
/// itself to the temp folder and runs from there.
pub fn temp_copy(me: &Path) -> Result<PathBuf> {
    let copy = std::env::temp_dir().join(format!("exile-uninstall-{}.exe", std::process::id()));
    fs::copy(me, &copy).context("preparing to uninstall")?;
    Ok(copy)
}

/// Whether `path` is inside `dir`, ignoring case as Windows does.
pub fn is_inside(path: &Path, dir: &Path) -> bool {
    let lower = |p: &Path| p.display().to_string().to_lowercase();
    Path::new(&lower(path)).starts_with(lower(dir))
}

/// Deletes the temp copy once it has exited.
pub fn delete_when_closed(me: &Path) {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // ping waits about two seconds, then the exe is no longer in use.
    let script = format!("ping -n 3 127.0.0.1 >NUL & del /F /Q \"{}\"", me.display());
    let _ = std::process::Command::new("cmd").args(["/D", "/C"]).raw_arg(&script).creation_flags(CREATE_NO_WINDOW).spawn();
}

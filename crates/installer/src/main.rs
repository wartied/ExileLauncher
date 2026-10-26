//! Exile's installer for Windows (exile-setup.exe). It carries exile.exe
//! (see `payload`), puts it in %LOCALAPPDATA%\Programs\Exile, adds Start menu
//! and desktop shortcuts and lists Exile in Settings > Apps. The copy it
//! leaves there as uninstall.exe removes it all again.
//!
//! exile-setup.exe [--quiet]            install (--quiet: no window, desktop shortcut included)
//! uninstall.exe [--uninstall] [--quiet] uninstall (--quiet: no window, data kept)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Only the payload format is built elsewhere, for its tests.
#![cfg_attr(not(windows), allow(dead_code))]

mod payload;
#[cfg(windows)]
mod setup;
#[cfg(windows)]
mod system;
#[cfg(windows)]
mod ui;

#[cfg(not(windows))]
fn main() {
    eprintln!("Exile's installer is for Windows. On Linux, get exile-linux.tar.gz from https://exiles.dev.");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    use std::process::ExitCode;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let (Ok(places), Ok(me)) = (system::Places::find(), std::env::current_exe()) else {
        return ExitCode::FAILURE;
    };
    let found = std::fs::File::open(&me).ok().and_then(|mut f| payload::find(&mut f).ok().flatten());

    let mode = match found {
        Some(payload) if !flag("--uninstall") => ui::Mode::Install { me: me.clone(), payload },
        _ => {
            // uninstall.exe can't delete itself while it runs, so a copy in
            // the temp folder does the work and deletes itself afterwards.
            if setup::is_inside(&me, &places.dir) {
                let mut rerun = args.clone();
                rerun.extend(["--uninstall".to_owned(), "--temp-copy".to_owned()]);
                let ok = setup::temp_copy(&me).and_then(|copy| Ok(std::process::Command::new(copy).args(rerun).spawn()?));
                return if ok.is_ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE };
            }
            ui::Mode::Uninstall
        }
    };

    let ok = if flag("--quiet") {
        let quiet = |_: f32, _: &str| {};
        match &mode {
            ui::Mode::Install { me, payload } => setup::install(&places, me, payload, true, &quiet).is_ok(),
            ui::Mode::Uninstall => setup::uninstall(&places, false, &quiet).is_ok(),
        }
    } else {
        ui::run(mode, places).is_ok()
    };
    if flag("--temp-copy") {
        setup::delete_when_closed(&me);
    }
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

# Exile

A fast, native Minecraft: Java Edition launcher, and a performance client to go with it.

**Download:** [exiles.dev](https://exiles.dev) (Windows installer or portable, Linux)

## Launcher

- **Native and light.** No web view or Electron: one small executable that uses no CPU while idle.
- **Quick to start.** Tuned Java settings, a startup cache built in the background, and a sign-in
  that renews itself before you press Play.
- **Safe memory.** Limits fit each Minecraft version, Windows always keeps a reserve, and a game
  won't start if it wouldn't fit in your RAM.
- **Mods.** One-click Fabric setup, then search and install mods from Modrinth, with dependencies
  handled for you.
- **Accounts.** Microsoft sign-in, several accounts with a quick switcher, and a 3D view of your
  skin, cape and badges.
- **Stays current.** Signed updates for the launcher and client together, and a changelog in
  Settings.

## Client

A Fabric mod for Minecraft 26.3. The launcher installs it for you, and it also works in any Fabric
launcher. Details in [client/README.md](client/README.md).

- **Measured optimisations.** Each one can be switched off, and ships only if it's measured to help.
- **Frame Budget.** Shows where each frame's time goes, and can hold a target frame rate.
- **Exile Bench.** A repeatable benchmark with average FPS, 1% and 0.1% lows.
- **HUD.** FPS, coordinates, armour, saturation, effects, keystrokes and CPS, laid out by dragging.
- **Badges.** Exile, Developer, OG name and Early supporter badges next to player names, on name
  tags and in the player list. You choose which of yours show, in the launcher.
- **Quality of life.** Zoom, free look, toggle sprint and sneak, fullbright, clear water and lava,
  lower fire and shield, slim or invisible armour, your own name tag in third person, and menu themes.

## Built with

- **Launcher:** Rust, with [egui](https://github.com/emilk/egui) for the interface.
- **Client:** Java 25 on [Fabric](https://fabricmc.net).
- **Type:** [Iosevka Aile](https://github.com/be5invis/Iosevka).

## Disclosures

- **Not official.** Exile is not an official Minecraft product and is not associated with Mojang or
  Microsoft. You need to own Minecraft: Java Edition, and Exile checks that before a game starts.
- **Your account.** You sign in on Microsoft's own page; Exile never sees your password. The session
  is stored on your computer (encrypted with Windows DPAPI, or readable only by you on Linux) and is
  sent only to Microsoft, Xbox and Mojang to sign you in.
- **No tracking.** Exile has no analytics or telemetry. It connects to Microsoft, Xbox and Mojang
  (sign-in, game files, skins), Fabric (the mod loader), Modrinth (mods), GitHub (release notes)
  and exiles.dev (updates and badges), and only when a feature needs them.
- **Badges.** To show badges, Exile signs in to exiles.dev (once per game, and from the launcher's
  Account settings). It proves who you are with what Mojang has already signed: your account's chat
  key and your profile. Your Minecraft login and private key are never sent to exiles.dev. It keeps your
  Minecraft UUID and name, when you first and last used Exile (which decides Early supporter), and
  which badges you've hidden. To show other players' badges, the client asks exiles.dev about the
  players it sees. Turning Badges off in the Exile menu (Features) stops both.
- **Game files.** Minecraft is downloaded from Mojang's servers and checked against Mojang's
  checksums. Exile doesn't redistribute any of it. To start faster, it runs the game from a local
  copy of the game jar without Mojang's signature; the downloaded original is kept unchanged.
- **Java settings.** Exile picks the garbage collector and memory flags itself (ZGC on machines with
  spare cores), which is why the game's memory use can look different from other launchers.
- **Server rules.** Fullbright, clear water and lava, and lower fire only change what you see, but
  some servers don't allow them. Check a server's rules before using them there.
- **Updates.** Every update is signed with Exile's release key and checked before it installs. By
  default Exile asks before updating.
- **Unsigned executables.** The Windows downloads aren't code-signed, so Windows SmartScreen may warn
  on first run. Every release is built from this repository by GitHub Actions.

## Building

Requires stable Rust, and JDK 25 for the client.

```
cd client && ./gradlew build && cd ..     # build the client first so the launcher bundles it
cargo run --release -p exile-launcher
```

`cargo build --release -p exile-installer` builds the Windows installer; the release workflow packs
the launcher into it. You may also run `cargo run --release -p exile-launcher`for the standalone Exile launcher.
`cargo test` runs the tests.

Data lives in `%APPDATA%\Exile` on Windows and `~/.local/share/Exile` on Linux; set `EXILE_HOME` to
use another folder.

**Forks:** please register your own Microsoft app instead of reusing Exile's ID, and use your own
name and logo. Set the ID in `crates/launcher/src/instance.rs`, with `microsoft_client_id` in
`settings.toml`, or with `EXILE_MSA_CLIENT_ID`. The app needs **Personal Microsoft accounts only**
and a **Public client/native** redirect URI of `http://localhost`. CurseForge search needs an API key
(`curseforge_api_key` or `EXILE_CURSEFORGE_KEY`); keep it out of the repository.

## License

Copyright (C) 2026 the Exile contributors.

Exile is free software under the [GNU General Public License, version 3](LICENSE). You can use,
study, share and change it; versions you distribute must stay under the same license with their
source available.

Iosevka Aile is licensed under the SIL Open Font License 1.1
(`crates/ui/assets/fonts/LICENSE-Iosevka.md`). Minecraft is a trademark of Mojang Synergies AB.

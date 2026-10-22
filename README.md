# Exile Launcher

A fast, native Minecraft: Java Edition launcher written in Rust. A performance client is planned next.

- **Native and light.** No web view and no Electron. The UI is drawn with egui, uses no CPU while idle, and ships as one ~9 MB executable.
- **Every version from 1.7.2 on**, each in its own instance with its own worlds, settings and mods. Several instances can run at once.
- **Safe memory handling.** Memory limits depend on the version and on how many mods it has, Windows always keeps a reserve, and a game won't start if everything running wouldn't fit in your RAM.
- **Mods.** One-click Fabric setup, then search and install mods from Modrinth (and CurseForge, with an API key). Dependencies install automatically, and conflicts, missing dependencies and hardware limits are flagged.
- **Microsoft sign-in.** Players sign in through Microsoft's own login page, and Exile checks the account owns Minecraft before the game can start.

```
crates/
  launcher/   Exile Launcher
```

## Build and run

Requires a recent stable Rust toolchain.

```
cargo run --release -p exile-launcher
```

The binary is `target/release/exile.exe`. Release builds don't open a console window. Java doesn't need
to be installed: Exile downloads the exact runtime Mojang ships for each Minecraft version.

## Microsoft sign-in

**Forks:** please register your own app rather than reusing Exile's ID. Set it in
`crates/launcher/src/instance.rs`, or at runtime with `microsoft_client_id = "<id>"` in `settings.toml`
or the `EXILE_MSA_CLIENT_ID` environment variable. The registration needs:

- Supported account types: **Personal Microsoft accounts only**
- Redirect URI: **Public client/native (mobile & desktop)**, `http://localhost`

## Mods

Mods run on [Fabric](https://fabricmc.net) (Minecraft 1.14 and newer). Each instance has its own
`minecraft/mods` folder. Disabling a mod renames it to `*.jar.disabled`, and `mods.toml` in the
instance folder records where each file came from.

Modrinth works out of the box. CurseForge requires an API key from
<https://console.curseforge.com>: set `curseforge_api_key = "<key>"` in `settings.toml`, set
`EXILE_CURSEFORGE_KEY`, or provide it at build time. Keep the key out of the repository.

## Data

Everything lives in `%APPDATA%\Exile` (Windows). Set `EXILE_HOME` to use another folder (portable mode).
The sign-in session is encrypted with Windows DPAPI and never leaves your computer.

## Tests

```
cargo test
```

Opt-in end-to-end checks against the live services (they download the game):

```
set EXILE_HOME=C:\some\scratch\folder
set EXILE_SMOKE_VERSION=1.21.1
set EXILE_SMOKE_MODS=sodium,modmenu
cargo test -- --ignored --test-threads=1 smoke
```

## License

Copyright (C) 2026 the Exile contributors.

Exile is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License, version 3, as published by the Free Software Foundation.
See [LICENSE](LICENSE).

The bundled [Iosevka Aile](https://github.com/be5invis/Iosevka) font (subset) is licensed under the
SIL Open Font License 1.1. See `crates/launcher/assets/fonts/LICENSE-Iosevka.md`.

Exile is not an official Minecraft product. It is not approved by or associated with Mojang or Microsoft.

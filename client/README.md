# Exile Client

A performance-first Fabric mod for Minecraft: Java Edition 26.3. It's the in-game half of Exile; the
[launcher](../README.md) installs and updates it, but it also works on its own in any Fabric launcher.

## What it does

- **Measured optimisations.** Every technique can be switched on or off and is measured (Exile Bench for
  frame rate, CPU time and GPU power for the background cap) before it ships. The full list and the
  measurements are in the in-game Performance tab.
- **Frame Budget.** A live breakdown of where each frame's time goes (game logic, world prep, world
  draw, HUD, GPU and present, frame cap, garbage collection), plus an optional governor that holds a
  target frame rate by trimming the least noticeable settings first.
- **Exile Bench.** A repeatable benchmark: a fixed flight through a fixed-seed world with fixed
  settings. It reports average FPS, 1% and 0.1% lows, frame-time percentiles and GC activity, and
  saves JSON results to `.minecraft/exile/bench`.
- **HUD.** FPS, coordinates, armour durability, food and saturation, potion effects, keystrokes, CPS
  and the Frame Budget overlay. In the layout editor, drag elements to move them, resize them (50% to
  200%), and choose what effects, armour and saturation show: full, icon only or text only. Saturation
  is also shown on the hunger bar itself, as gold outlines on the drumsticks it still backs.
- **Visibility.** Fullbright (with an optional toggle key), clear water and clear lava, and fire and
  shield drawn lower on screen.
- **Menu themes.** Vanilla, Dark, or Exile's own blue-grey for Minecraft's menus and inventories. It
  tints rather than replaces textures, so it works with resource packs.
- **Features.** Free look (hold Left Alt), zoom (hold C; amount, toggle or hold, smooth, scroll to adjust
  and cinematic camera are all adjustable), toggle sprint and toggle sneak. Keys can be rebound right in
  the Exile menu.
- **Armour.** Slim armour (armour fitted closer to the body, on every player) and invisible armour (hide
  your own armour so your skin shows). Both only change what you see; other players see your armour as
  normal.
- **Mod list in the pause menu.** Exile's own minimal mod list, which opens other mods' settings
  screens through the Mod Menu API, so Mod Menu isn't needed. If Mod Menu is installed anyway, the
  real one takes over automatically.

Open the Exile menu from the pause menu or with Right Shift.

## Building

Requires JDK 25.

```
cd client
./gradlew build          # jar in build/libs/
./gradlew :runClient     # run the game with Exile (plus Sodium and Lithium for testing)
```

Run the benchmark unattended (the game quits when it's done):

```
./gradlew :runClient -PexileArgs="-Dexile.bench=auto -Dexile.bench.label=mytest"
```

Further options, passed as `-D` properties through `exileArgs`:

| Property | Meaning |
|---|---|
| `exile.bench.scene=entities` | Adds 400 still animals along the flight path |
| `exile.opt.<name>=false` | Turns one optimisation off (A/B testing) |
| `exile.bench.scene=travel` | Flies straight ahead instead of circling, so new chunks load all the time |
| `exile.opt.<name>=false` | Turns one optimisation off (A/B testing) |
| `exile.opt.all=false` | Turns every optimisation off |

### Comparing configurations

`tools/` has the scripts used to measure everything Exile ships (Git Bash on Windows, or any POSIX
shell, plus Python 3):

```
tools/freeze.sh                                   # copy the client to a frozen benchmark folder
tools/bench.sh zgc-r1 -Xmx4G -XX:+UseZGC          # one unattended bench, label then JVM args
tools/summary.py zgc g1                           # mean and spread per label (runs named <label>-r<n>)
tools/startup.sh base-r1 -Xmx4G                   # milliseconds from JVM start to the first menu
```

Benchmarks run from the frozen copy (default `%LOCALAPPDATA%/exile-dev/bench-client`, override with
`EXILE_BENCH_DIR`), so editing the source during a series of runs doesn't change what's measured.
Throw away the first run after freezing: it builds the jar and generates the bench world. Use three or
more runs per configuration and compare the spread, not just the means.

## Layout

```
src/main/java/dev/exile/client/
  bench/      Exile Bench
  perf/       FrameStats (frame timing), optimisations, Frame Budget governor
  hud/        HUD system and elements
  ui/         Exile menu, layout editor, widgets and theme
  feature/    free look, keybinds, click counter
  mixin/      the hooks into Minecraft
modmenu-shim/ Mod Menu API interfaces (nested in the jar, replaced by real Mod Menu if present)
tools/        benchmark and startup scripts
```

## License

GPL-3.0-only, like the rest of Exile. The bundled Iosevka Aile font is under the SIL Open Font License
1.1 (`LICENSE-Iosevka.md`).

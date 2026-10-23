//! Memory policy.
//!
//! Every allocation decision goes through this module so there is exactly one
//! place that decides how much heap a game may get. The rules:
//!
//! * Each Minecraft generation has a minimum heap it needs to run, a sensible
//!   default and a ceiling past which more heap only makes GC pauses longer.
//! * The OS always keeps a reserve (a quarter of RAM, clamped to 1.5–4 GB).
//! * A JVM uses memory beyond `-Xmx` (metaspace, code cache, thread stacks,
//!   LWJGL/OpenGL native buffers). We account for that as "overhead" so that
//!   the *real* footprint of every running game fits in the budget.
//! * Before a launch, the footprints of all games already started by Exile plus
//!   the new one must fit in `total - reserve`. If they don't, the launch is
//!   blocked rather than letting the system start swapping.

use sysinfo::{MemoryRefreshKind, RefreshKind, System};

pub const MB: u64 = 1024 * 1024;
/// Granularity of the memory slider and of every value we hand to the JVM.
pub const STEP_MB: u64 = 256;

#[derive(Clone, Copy, Debug)]
pub struct SystemMemory {
    pub total_mb: u64,
    pub available_mb: u64,
}

pub fn probe() -> SystemMemory {
    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    SystemMemory {
        total_mb: sys.total_memory() / MB,
        available_mb: sys.available_memory() / MB,
    }
}

/// What a given Minecraft version needs, independent of the machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameNeeds {
    pub min_mb: u64,
    pub recommended_mb: u64,
    pub max_useful_mb: u64,
}

/// `release_time` is the ISO-8601 timestamp from the version manifest; ISO
/// timestamps compare correctly as strings.
pub fn needs_for(release_time: &str) -> GameNeeds {
    const V1_13: &str = "2018-07-18";
    const V1_18: &str = "2021-11-30";
    if release_time < V1_13 {
        GameNeeds { min_mb: 1024, recommended_mb: 2048, max_useful_mb: 4096 }
    } else if release_time < V1_18 {
        GameNeeds { min_mb: 1536, recommended_mb: 3072, max_useful_mb: 6144 }
    } else {
        GameNeeds { min_mb: 1536, recommended_mb: 4096, max_useful_mb: 8192 }
    }
}

/// Raises the needs for an instance with `mods` enabled mods. A handful of
/// performance mods changes nothing; large packs need far more heap.
/// Rough rule from common modpack guidance: +32 MB recommended and +16 MB
/// minimum per mod beyond the first ten, capped at +4 GB / +1.5 GB.
pub fn with_mods(needs: GameNeeds, mods: usize) -> GameNeeds {
    let extra = (mods as u64).saturating_sub(10);
    if mods == 0 {
        return needs;
    }
    let round_up = |mb: u64| mb.div_ceil(STEP_MB) * STEP_MB;
    GameNeeds {
        min_mb: round_up(needs.min_mb + (extra * 16).min(1536)),
        recommended_mb: round_up(needs.recommended_mb + (extra * 32).min(4096)),
        max_useful_mb: (needs.max_useful_mb + mods as u64 * 64).min(16384),
    }
}

/// Memory the OS and other programs keep no matter what.
pub fn os_reserve_mb(total_mb: u64) -> u64 {
    (total_mb / 4).clamp(1536, 4096)
}

/// Native memory a JVM running Minecraft uses on top of its heap.
pub fn jvm_overhead_mb(heap_mb: u64) -> u64 {
    384 + heap_mb / 8
}

/// Worst-case resident size of a game launched with `heap_mb` of heap.
pub fn footprint_mb(heap_mb: u64) -> u64 {
    heap_mb + jvm_overhead_mb(heap_mb)
}

fn round_down(mb: u64) -> u64 {
    mb / STEP_MB * STEP_MB
}

/// Slider bounds for one version on this machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub min_mb: u64,
    pub max_mb: u64,
    pub recommended_mb: u64,
    /// False when this machine cannot give the version its minimum heap while
    /// keeping the OS reserve. The game is then not launchable.
    pub fits: bool,
}

pub fn limits(total_mb: u64, needs: GameNeeds) -> Limits {
    let budget = total_mb.saturating_sub(os_reserve_mb(total_mb));
    // Largest heap h with h + 384 + h/8 <= budget.
    let mut max_heap = round_down(budget.saturating_sub(384) * 8 / 9);
    // A 32-bit JVM cannot address much more than ~1.5 GB of heap.
    if cfg!(target_pointer_width = "32") {
        max_heap = max_heap.min(1024);
    }
    let max_mb = round_down(needs.max_useful_mb.min(max_heap));
    let fits = max_mb >= needs.min_mb;
    let max_mb = max_mb.max(needs.min_mb);
    Limits {
        min_mb: needs.min_mb,
        max_mb,
        recommended_mb: needs.recommended_mb.clamp(needs.min_mb, max_mb),
        fits,
    }
}

/// Snaps a stored allocation onto this machine's limits. Instances can be
/// copied between machines, and settings files can be hand-edited, so the
/// stored value is never trusted as-is.
pub fn sanitize(heap_mb: u64, limits: Limits) -> u64 {
    round_down(heap_mb).clamp(limits.min_mb, limits.max_mb)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchCheck {
    Ok,
    /// Launch is possible but the user should confirm.
    Warn(String),
    Blocked(String),
}

/// Decides whether a game with `heap_mb` may start right now.
/// `running_footprint_mb` is the sum of [`footprint_mb`] for every game Exile
/// already has running.
pub fn check_launch(
    heap_mb: u64,
    limits: Limits,
    sys: SystemMemory,
    running_footprint_mb: u64,
) -> LaunchCheck {
    if !limits.fits {
        return LaunchCheck::Blocked(format!(
            "This version needs at least {} of memory, but this computer can only spare {} \
             while leaving enough for Windows.",
            fmt_mb(limits.min_mb),
            fmt_mb(limits.max_mb.min(sys.total_mb.saturating_sub(os_reserve_mb(sys.total_mb)))),
        ));
    }
    let budget = sys.total_mb.saturating_sub(os_reserve_mb(sys.total_mb));
    let need = footprint_mb(heap_mb);
    if running_footprint_mb + need > budget {
        let left = budget.saturating_sub(running_footprint_mb);
        return LaunchCheck::Blocked(format!(
            "Not enough memory to run this alongside your other instances. \
             This instance needs about {} in total, but only {} is left. \
             Close an instance or lower its memory.",
            fmt_mb(need),
            fmt_mb(left),
        ));
    }
    if need > sys.available_mb {
        return LaunchCheck::Warn(format!(
            "Other programs are using a lot of memory. Only {} is free right now and this \
             instance may use up to {}. Your computer may slow down.",
            fmt_mb(sys.available_mb),
            fmt_mb(need),
        ));
    }
    LaunchCheck::Ok
}

/// JVM memory arguments for a heap size. `-Xms` stays small so that several
/// instances don't each commit their full heap up front; the heap grows as the
/// game needs it and G1 hands unused regions back.
pub fn jvm_args(heap_mb: u64) -> Vec<String> {
    let xms = heap_mb.min(512);
    vec![format!("-Xms{xms}M"), format!("-Xmx{heap_mb}M")]
}

pub fn fmt_mb(mb: u64) -> String {
    if mb >= 1024 {
        let tenths = (mb as f64 / 1024.0 * 10.0).round() as u64;
        if tenths.is_multiple_of(10) { format!("{} GB", tenths / 10) } else { format!("{}.{} GB", tenths / 10, tenths % 10) }
    } else {
        format!("{mb} MB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODERN: &str = "2024-06-13T08:24:03+00:00";
    const OLD: &str = "2014-06-26T10:00:00+00:00";

    #[test]
    fn needs_by_generation() {
        assert_eq!(needs_for(OLD).min_mb, 1024);
        assert_eq!(needs_for("2019-12-10").recommended_mb, 3072);
        assert_eq!(needs_for(MODERN).recommended_mb, 4096);
    }

    #[test]
    fn limits_keep_os_reserve() {
        for total in [4096, 6144, 8192, 16384, 32768, 65536] {
            let l = limits(total, needs_for(MODERN));
            if l.fits {
                assert!(footprint_mb(l.max_mb) + os_reserve_mb(total) <= total, "total={total}");
            }
            assert!(l.min_mb <= l.recommended_mb && l.recommended_mb <= l.max_mb);
            assert_eq!(l.max_mb % STEP_MB, 0);
        }
    }

    #[test]
    fn small_machine_still_runs() {
        assert!(limits(4096, needs_for(OLD)).fits);
        assert!(limits(4096, needs_for(MODERN)).fits);
        assert!(!limits(3072, needs_for(MODERN)).fits);
    }

    #[test]
    fn big_machine_caps_at_useful() {
        let l = limits(65536, needs_for(MODERN));
        assert_eq!(l.max_mb, 8192);
        assert_eq!(l.recommended_mb, 4096);
    }

    #[test]
    fn sanitize_clamps_and_rounds() {
        let l = limits(16384, needs_for(MODERN));
        assert_eq!(sanitize(100, l), l.min_mb);
        assert_eq!(sanitize(1_000_000, l), l.max_mb);
        assert_eq!(sanitize(3000, l), 2816);
    }

    #[test]
    fn budget_blocks_overcommit() {
        let sys = SystemMemory { total_mb: 16384, available_mb: 12000 };
        let l = limits(sys.total_mb, needs_for(MODERN));
        assert_eq!(check_launch(4096, l, sys, 0), LaunchCheck::Ok);
        let two = footprint_mb(4096) * 2;
        assert!(matches!(check_launch(4096, l, sys, two), LaunchCheck::Blocked(_)));
    }

    #[test]
    fn low_free_memory_warns() {
        let sys = SystemMemory { total_mb: 16384, available_mb: 2000 };
        let l = limits(sys.total_mb, needs_for(MODERN));
        assert!(matches!(check_launch(4096, l, sys, 0), LaunchCheck::Warn(_)));
    }

    #[test]
    fn mods_raise_needs() {
        let base = needs_for(MODERN);
        assert_eq!(with_mods(base, 0), base);
        // A small performance set doesn't change the recommendation.
        assert_eq!(with_mods(base, 8).recommended_mb, base.recommended_mb);
        let pack = with_mods(base, 150);
        assert_eq!(pack.recommended_mb, base.recommended_mb + 4096);
        assert!(pack.min_mb > base.min_mb && pack.min_mb.is_multiple_of(STEP_MB));
        assert!(pack.max_useful_mb <= 16384);
    }

    #[test]
    fn formatting() {
        assert_eq!(fmt_mb(512), "512 MB");
        assert_eq!(fmt_mb(4096), "4 GB");
        assert_eq!(fmt_mb(2816), "2.8 GB");
    }
}

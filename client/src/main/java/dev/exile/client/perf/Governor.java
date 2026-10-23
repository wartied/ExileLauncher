package dev.exile.client.perf;

import com.mojang.blaze3d.platform.FramerateLimitTracker;
import dev.exile.client.ExileClient;
import dev.exile.client.bench.ExileBench;
import dev.exile.client.config.ExileConfig;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import net.minecraft.client.CloudStatus;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.server.level.ParticleStatus;

/**
 * Frame Budget governor: holds a target frame rate by trimming the settings
 * players notice least, one step at a time, and restoring them when there is
 * headroom again.
 *
 * <p>Order of steps: entity distance, particles, clouds, then render distance
 * in steps of two chunks (never below 6). It reacts slowly on purpose (two
 * seconds below target to step down, six seconds well above to step back up,
 * longer cooldowns around render distance, which rebuilds chunks) so it never
 * oscillates. The player's own settings are remembered in the config and put
 * back whenever the governor stops, and on the next start if the game closed
 * while it was active.
 */
public final class Governor {
	private static final int MIN_RENDER_DISTANCE = 6;
	private static final String RD = "render_distance", ENTITY = "entity_distance", PARTICLES = "particles", CLOUDS = "clouds";

	/** One quality step; {@code revert} restores exactly what {@code apply} changed. */
	private record Step(String label, Runnable apply, Runnable revert) {}

	private static final List<Step> applied = new ArrayList<>();
	private static List<Step> plan = List.of();
	private static boolean active;
	private static long lastFrames;
	private static long lastSampleNs;
	private static int lowSeconds, highSeconds;
	private static long cooldownUntil;

	private Governor() {}

	private static ExileConfig.Performance cfg() {
		return ExileConfig.get().performance;
	}

	/** Called once at startup: undo anything a previous session left applied. */
	public static void restoreLeftovers(Minecraft mc) {
		if (!cfg().governorBaseline.isEmpty()) {
			ExileClient.LOG.info("Frame Budget governor: restoring settings from the last session");
			restoreBaseline(mc);
		}
	}

	public static void tick(Minecraft mc) {
		boolean want = cfg().governor && mc.level != null && !ExileBench.running();
		if (want != active) {
			if (want) start(mc); else stop(mc);
		}
		if (!active) return;

		long now = System.nanoTime();
		if (now - lastSampleNs < 1_000_000_000L) return;
		long frames = FrameStats.totalFrames();
		double fps = (frames - lastFrames) / ((now - lastSampleNs) / 1e9);
		lastFrames = frames;
		lastSampleNs = now;

		// Only judge real gameplay: menus, throttled windows and pauses run slow on purpose.
		if (mc.gui.screen() != null || mc.isPaused()
			|| mc.getFramerateLimitTracker().getThrottleReason() != FramerateLimitTracker.FramerateThrottleReason.NONE
			|| !mc.getWindow().isFocused()) {
			lowSeconds = highSeconds = 0;
			return;
		}
		int target = effectiveTarget(mc);
		if (fps < target * 0.95) {
			lowSeconds++;
			highSeconds = 0;
		} else if (fps > target * 1.30) {
			highSeconds++;
			lowSeconds = 0;
		} else {
			lowSeconds = highSeconds = 0;
		}
		if (now < cooldownUntil) return;

		if (lowSeconds >= 2 && applied.size() < plan.size()) {
			Step step = plan.get(applied.size());
			step.apply.run();
			applied.add(step);
			lowSeconds = 0;
			cooldownUntil = now + (step.label.startsWith("Render") ? 8_000_000_000L : 3_000_000_000L);
		} else if (highSeconds >= 6 && !applied.isEmpty()) {
			// Undo only the newest step: touching other settings (render distance
			// above all) would rebuild chunks for nothing.
			Step undone = applied.removeLast();
			undone.revert.run();
			highSeconds = 0;
			cooldownUntil = now + (undone.label.startsWith("Render") ? 10_000_000_000L : 6_000_000_000L);
		}
	}

	/** The target, capped by vsync and the player's own frame limit. */
	private static int effectiveTarget(Minecraft mc) {
		int target = cfg().governorTargetFps;
		int limit = mc.options.framerateLimit().get();
		if (limit < 260) target = Math.min(target, limit);
		if (mc.options.enableVsync().get()) {
			var monitor = mc.getWindow().findBestMonitor();
			float refresh = monitor == null ? 0 : monitor.currentMode().getRefreshRate();
			if (refresh > 0) target = Math.min(target, Math.round(refresh));
		}
		return target;
	}

	private static void start(Minecraft mc) {
		Options o = mc.options;
		Map<String, Double> base = cfg().governorBaseline;
		base.clear();
		base.put(RD, (double) o.renderDistance().get());
		base.put(ENTITY, o.entityDistanceScaling().get());
		base.put(PARTICLES, (double) o.particles().get().ordinal());
		base.put(CLOUDS, (double) o.cloudStatus().get().ordinal());
		ExileConfig.get().save();
		plan = buildPlan(o);
		applied.clear();
		lowSeconds = highSeconds = 0;
		lastFrames = FrameStats.totalFrames();
		lastSampleNs = System.nanoTime();
		cooldownUntil = lastSampleNs + 3_000_000_000L;
		active = true;
	}

	private static void stop(Minecraft mc) {
		active = false;
		applied.clear();
		restoreBaseline(mc);
	}

	/**
	 * Builds the steps in order. Each step records the value its knob had
	 * before it, so reverting the newest step puts back exactly that value.
	 */
	private static List<Step> buildPlan(Options o) {
		List<Step> steps = new ArrayList<>();
		double entity = o.entityDistanceScaling().get();
		ParticleStatus particles = o.particles().get();
		CloudStatus clouds = o.cloudStatus().get();

		if (entity > 0.75) {
			double before = entity;
			steps.add(new Step("Entity distance 75%", () -> o.entityDistanceScaling().set(0.75), () -> o.entityDistanceScaling().set(before)));
			entity = 0.75;
		}
		if (particles == ParticleStatus.ALL) {
			steps.add(new Step("Particles decreased", () -> o.particles().set(ParticleStatus.DECREASED), () -> o.particles().set(ParticleStatus.ALL)));
			particles = ParticleStatus.DECREASED;
		}
		if (entity > 0.5) {
			double before = entity;
			steps.add(new Step("Entity distance 50%", () -> o.entityDistanceScaling().set(0.5), () -> o.entityDistanceScaling().set(before)));
		}
		if (clouds != CloudStatus.OFF) {
			steps.add(new Step("Clouds off", () -> o.cloudStatus().set(CloudStatus.OFF), () -> o.cloudStatus().set(clouds)));
		}
		if (particles != ParticleStatus.MINIMAL) {
			ParticleStatus before = particles;
			steps.add(new Step("Particles minimal", () -> o.particles().set(ParticleStatus.MINIMAL), () -> o.particles().set(before)));
		}
		int rd = o.renderDistance().get();
		for (int d = rd - 2; d >= MIN_RENDER_DISTANCE; d -= 2) {
			int value = d, before = d + 2;
			steps.add(new Step("Render distance " + value, () -> o.renderDistance().set(value), () -> o.renderDistance().set(before)));
		}
		return steps;
	}

	private static void restoreBaseline(Minecraft mc) {
		restoreBaseline(mc, true);
	}

	private static void restoreBaseline(Minecraft mc, boolean forget) {
		Map<String, Double> base = cfg().governorBaseline;
		if (base.isEmpty()) return;
		Options o = mc.options;
		if (base.containsKey(RD)) o.renderDistance().set(base.get(RD).intValue());
		if (base.containsKey(ENTITY)) o.entityDistanceScaling().set(base.get(ENTITY));
		if (base.containsKey(PARTICLES)) o.particles().set(ParticleStatus.values()[clampIndex(base.get(PARTICLES), ParticleStatus.values().length)]);
		if (base.containsKey(CLOUDS)) o.cloudStatus().set(CloudStatus.values()[clampIndex(base.get(CLOUDS), CloudStatus.values().length)]);
		if (forget) {
			base.clear();
			ExileConfig.get().save();
			o.save();
		}
	}

	private static int clampIndex(double v, int size) {
		return Math.clamp((int) v, 0, size - 1);
	}

	/** One-line state for the Frame Budget overlay, or null when off. */
	public static String status() {
		if (!active) return null;
		String head = "Governor  " + cfg().governorTargetFps + " fps target";
		return applied.isEmpty() ? head + "  ·  full quality" : head + "  ·  " + applied.getLast().label;
	}

	/** Called when the game shuts down: never leave reduced settings behind. */
	public static void shutdown(Minecraft mc) {
		if (active) stop(mc);
	}
}

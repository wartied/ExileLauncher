package dev.exile.client.feature;

import dev.exile.client.config.ExileConfig;
import net.minecraft.client.Minecraft;

/**
 * Zoom: narrows the world's field of view while the key is held (or until it
 * is pressed again, in toggle mode). The hand keeps its normal field of view,
 * mouse sensitivity drops with the magnification so aiming stays steady, and
 * the mouse wheel can change the magnification while zoomed.
 */
public final class Zoom {
	private static final float MIN_LEVEL = 1.5f, MAX_LEVEL = 50f;
	/** How fast smooth zoom closes the gap, per second (higher = snappier). */
	private static final float EASE = 14f;

	private static boolean toggled;
	private static boolean active;
	/** Target magnification while zoomed; starts at the configured level, then follows the wheel. */
	private static float level = 4f;
	/** Magnification on screen this frame (eases towards the target). */
	private static float current = 1f;
	private static long lastFrameNs;
	/** Whether Exile switched the cinematic camera on (so it only switches off what it switched on). */
	private static boolean cinematicSet;

	private Zoom() {}

	private static ExileConfig.Features cfg() {
		return ExileConfig.get().features;
	}

	/** Client tick: toggle presses and the cinematic camera. */
	public static void tick(Minecraft mc) {
		var f = cfg();
		while (Keybinds.zoom.consumeClick()) {
			if (f.zoomToggle && mc.gui.screen() == null) toggled = !toggled;
		}
		if (!f.zoom || !f.zoomToggle || mc.player == null) toggled = false;

		boolean wantCinematic = active && f.zoomCinematic;
		if (wantCinematic && !cinematicSet && !mc.options.smoothCamera) {
			mc.options.smoothCamera = true;
			cinematicSet = true;
		} else if (!wantCinematic && cinematicSet) {
			mc.options.smoothCamera = false;
			cinematicSet = false;
		}
	}

	/**
	 * Called once per frame while the camera is set up: updates the zoom and
	 * returns the factor to multiply the world's field of view by.
	 */
	public static float fovScale() {
		Minecraft mc = Minecraft.getInstance();
		var f = cfg();
		boolean want = f.zoom && mc.player != null && (f.zoomToggle ? toggled : Keybinds.zoom.isDown());
		if (want && !active) level = f.zoomLevel;
		active = want;

		long now = System.nanoTime();
		float dt = lastFrameNs == 0 ? 0f : Math.min((now - lastFrameNs) / 1e9f, 0.1f);
		lastFrameNs = now;
		float goal = active ? level : 1f;
		if (f.zoomSmooth) {
			current += (goal - current) * (1f - (float) Math.exp(-EASE * dt));
			if (Math.abs(goal - current) < 0.002f) current = goal;
		} else {
			current = goal;
		}
		return 1f / current;
	}

	/** Mouse sensitivity factor: slower the further in you are. */
	public static double sensitivityScale() {
		return 1.0 / current;
	}

	/** Mouse wheel while zoomed: change the magnification. Returns true if the scroll was used. */
	public static boolean scroll(double amount) {
		if (!active || !cfg().zoomScroll || amount == 0) return false;
		level = Math.clamp(level * (float) Math.pow(1.25, amount), MIN_LEVEL, MAX_LEVEL);
		return true;
	}
}

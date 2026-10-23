package dev.exile.client.perf;

import dev.exile.client.bench.ExileBench;
import dev.exile.client.config.ExileConfig;
import net.minecraft.client.Minecraft;

/**
 * Caps the frame rate while the game window is visible but not focused.
 *
 * <p>Vanilla already throttles a minimised window and an idle (AFK) player,
 * but a window you've merely clicked away from (alt-tabbed to a browser, or a
 * second Exile instance in the background) still renders flat out. With
 * several instances open that wastes most of the GPU on games nobody is
 * looking at. Exile caps such windows at a low rate (30 FPS by default),
 * which still looks live, and restores full speed the moment you click back.
 */
public final class BackgroundThrottle {
	public static final Tuning.Switch SWITCH = Tuning.register(
		"background_throttle",
		"Background frame cap",
		"Limit FPS while the window is unfocused",
		true
	);

	private BackgroundThrottle() {}

	/** Adjusts vanilla's frame limit for this frame. */
	public static int limit(Minecraft mc, int vanilla) {
		if (!SWITCH.enabled() || ExileBench.running() || mc.level == null) return vanilla;
		var window = mc.getWindow();
		if (window.isFocused() || window.isIconified()) return vanilla; // vanilla handles minimised
		return Math.min(vanilla, ExileConfig.get().performance.backgroundFps);
	}
}

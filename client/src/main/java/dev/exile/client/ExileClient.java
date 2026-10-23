package dev.exile.client;

import dev.exile.client.bench.ExileBench;
import dev.exile.client.feature.Keybinds;
import dev.exile.client.hud.ExileHud;
import dev.exile.client.perf.Governor;
import dev.exile.client.perf.Optimisations;
import dev.exile.client.ui.TitleScreenButton;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.gui.screens.AccessibilityOnboardingScreen;
import net.minecraft.client.gui.screens.TitleScreen;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class ExileClient implements ClientModInitializer {
	public static final String MOD_ID = "exile";
	public static final Logger LOG = LoggerFactory.getLogger("Exile");

	private static boolean startupLogged;
	/** -Dexile.bench=auto: run Exile Bench once the title screen shows, then quit. */
	private static boolean autoBenchPending = "auto".equals(System.getProperty("exile.bench"));

	@Override
	public void onInitializeClient() {
		Optimisations.init();
		Keybinds.init();
		ExileHud.init();
		TitleScreenButton.init();

		ClientLifecycleEvents.CLIENT_STARTED.register(Governor::restoreLeftovers);
		ClientLifecycleEvents.CLIENT_STOPPING.register(client -> {
			if (ExileBench.running()) ExileBench.abort("game closing");
			Governor.shutdown(client);
		});
		ClientTickEvents.END_CLIENT_TICK.register(client -> {
			// A fresh game folder shows the accessibility prompt before the title screen.
			var screen = client.gui.screen();
			if (!startupLogged && (screen instanceof TitleScreen || screen instanceof AccessibilityOnboardingScreen)) {
				startupLogged = true;
				long ms = System.currentTimeMillis() - java.lang.management.ManagementFactory.getRuntimeMXBean().getStartTime();
				LOG.info("Startup: menu reached {} ms after JVM start", ms);
				// -Dexile.startup.probe=true: measure startup only, then quit (used to test JVM startup options).
				if (Boolean.getBoolean("exile.startup.probe")) client.stop();
			}
			if (autoBenchPending && client.gui.overlay() == null && client.level == null
				&& (screen instanceof TitleScreen || screen instanceof AccessibilityOnboardingScreen)) {
				autoBenchPending = false;
				ExileBench.start(true, null);
			}
			ExileBench.tick(client);
			Keybinds.tick(client);
			ExileHud.tick(client);
			Governor.tick(client);
		});
		LOG.info("Exile Client loaded");
	}
}

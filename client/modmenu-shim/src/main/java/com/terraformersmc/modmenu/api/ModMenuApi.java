package com.terraformersmc.modmenu.api;

import java.util.Map;
import java.util.function.Consumer;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

/**
 * The "modmenu" entrypoint interface. Exile reads these entrypoints to show
 * a Configure button for mods that provide a config screen.
 */
public interface ModMenuApi {
	/** Opens Exile's mod list (looked up reflectively; the shim has no dependency on Exile). */
	static Screen createModsScreen(Screen previous) {
		try {
			return (Screen) Class.forName("dev.exile.client.ui.ExileMenuScreen")
				.getMethod("modsTab", Screen.class)
				.invoke(null, previous);
		} catch (ReflectiveOperationException e) {
			return previous;
		}
	}

	static Component createModsButtonText() {
		return Component.literal("Mods");
	}

	default ConfigScreenFactory<?> getModConfigScreenFactory() {
		return parent -> null;
	}

	default UpdateChecker getUpdateChecker() {
		return null;
	}

	default Map<String, ConfigScreenFactory<?>> getProvidedConfigScreenFactories() {
		return Map.of();
	}

	default Map<String, UpdateChecker> getProvidedUpdateCheckers() {
		return Map.of();
	}

	default void attachModpackBadges(Consumer<String> consumer) {
	}
}

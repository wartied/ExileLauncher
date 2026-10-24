package dev.exile.client.feature;

import com.mojang.blaze3d.platform.InputConstants;
import dev.exile.client.ui.ExileMenuScreen;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.minecraft.client.KeyMapping;
import dev.exile.client.config.ExileConfig;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;

/** Exile's keys: rebindable in the Exile menu, or under Controls in the Exile category. */
public final class Keybinds {
	private static final KeyMapping.Category CATEGORY = KeyMapping.Category.register(Identifier.fromNamespaceAndPath("exile", "exile"));
	public static KeyMapping freelook;
	public static KeyMapping zoom;
	public static KeyMapping fullbright;
	public static KeyMapping menu;

	private Keybinds() {}

	public static void init() {
		freelook = KeyMappingHelper.registerKeyMapping(new KeyMapping("key.exile.freelook", InputConstants.KEY_LALT, CATEGORY));
		zoom = KeyMappingHelper.registerKeyMapping(new KeyMapping("key.exile.zoom", InputConstants.KEY_C, CATEGORY));
		// Unbound by default: there's no free key everyone would agree on.
		fullbright = KeyMappingHelper.registerKeyMapping(new KeyMapping("key.exile.fullbright", InputConstants.UNKNOWN.getValue(), CATEGORY));
		menu = KeyMappingHelper.registerKeyMapping(new KeyMapping("key.exile.menu", InputConstants.KEY_RSHIFT, CATEGORY));
	}

	public static void tick(Minecraft mc) {
		Freelook.tick(mc, freelook.isDown());
		Zoom.tick(mc);
		while (fullbright.consumeClick()) {
			var f = ExileConfig.get().features;
			f.fullbright = !f.fullbright;
			ExileConfig.get().save();
			mc.gui.hud.setOverlayMessage(Component.literal(f.fullbright ? "Fullbright on" : "Fullbright off"), false);
		}
		while (menu.consumeClick()) {
			if (mc.gui.screen() == null) mc.gui.setScreen(new ExileMenuScreen(null));
		}
	}
}

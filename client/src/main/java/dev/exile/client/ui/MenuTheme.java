package dev.exile.client.ui;

import dev.exile.client.config.ExileConfig;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.resources.Identifier;
import net.minecraft.util.ARGB;

/**
 * Dark menus for vanilla Minecraft, by tinting rather than replacing
 * textures: every widget and container texture is multiplied by a dark
 * colour as it's drawn. That works with any resource pack, switches
 * instantly, and needs none of Mojang's art copied into Exile.
 *
 * <ul>
 *   <li>Dark: neutral greys.</li>
 *   <li>Exile: the launcher's blue-tinted greys, with the accent on hover.</li>
 * </ul>
 */
public final class MenuTheme {
	public enum Mode { VANILLA, DARK, EXILE }

	/** Vanilla's dark grey container labels ("Inventory", "Chest"), unreadable on a dark background. */
	private static final int LABEL = 0xFF404040;

	private MenuTheme() {}

	public static Mode mode() {
		return switch (ExileConfig.get().features.menuTheme) {
			case "dark" -> Mode.DARK;
			case "exile" -> Mode.EXILE;
			default -> Mode.VANILLA;
		};
	}

	public static void set(Mode mode) {
		ExileConfig.get().features.menuTheme = mode.name().toLowerCase();
	}

	/** Tint for a GUI sprite (buttons, sliders, text fields, tabs, slots...). */
	public static int sprite(Identifier id, int color) {
		Mode mode = mode();
		if (mode == Mode.VANILLA || !"minecraft".equals(id.getNamespace())) return color;
		String path = id.getPath();
		if (!(path.startsWith("widget/") || path.startsWith("container/") || path.startsWith("recipe_book/") || path.startsWith("popup/"))) {
			return color;
		}
		boolean hovered = path.contains("highlighted") || path.endsWith("_hover");
		return ARGB.multiply(color, tint(mode, hovered));
	}

	/** Tint for a whole GUI texture (inventory, chest and other container backgrounds). */
	public static int texture(Identifier id, int color) {
		Mode mode = mode();
		if (mode == Mode.VANILLA || !"minecraft".equals(id.getNamespace())) return color;
		String path = id.getPath();
		if (!(path.startsWith("textures/gui/container/") || path.startsWith("textures/gui/recipe_book"))) return color;
		return ARGB.multiply(color, tint(mode, false));
	}

	/** Container labels turn light when their background turns dark. */
	public static int text(int color) {
		if (color != LABEL || mode() == Mode.VANILLA) return color;
		if (!(Minecraft.getInstance().gui.screen() instanceof AbstractContainerScreen<?>)) return color;
		return mode() == Mode.EXILE ? Theme.TEXT : 0xFFE0E0E0;
	}

	private static int tint(Mode mode, boolean hovered) {
		return switch (mode) {
			// Vanilla's light grey #C6C6C6 becomes about #474747; hovered is a step lighter.
			case DARK -> hovered ? 0xFF808080 : 0xFF5C5C5C;
			// About #3A4456, Exile's surface blue-grey; hovered outlines take the accent.
			case EXILE -> hovered ? Theme.ACCENT : 0xFF4B5870;
			case VANILLA -> 0xFFFFFFFF;
		};
	}
}

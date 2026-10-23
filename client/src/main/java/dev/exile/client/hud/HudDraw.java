package dev.exile.client.hud;

import dev.exile.client.ui.Theme;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.network.chat.Component;

/** Small drawing helper for HUD elements. */
public final class HudDraw {
	public GuiGraphicsExtractor g;

	HudDraw() {}

	/** Looked up on use: the font doesn't exist yet when the mod initialises. */
	private Font font() {
		return Minecraft.getInstance().font;
	}

	public void text(Component c, int x, int y, int color) {
		g.text(font(), c, x, y, color, false);
	}

	public int width(Component c) {
		return font().width(c);
	}

	public void fill(int x, int y, int w, int h, int color) {
		g.fill(x, y, x + w, y + h, color);
	}

	/** Backdrop with softened corners. */
	public void backdrop(int x, int y, int w, int h) {
		int c = Theme.HUD_BACKDROP;
		g.fill(x + 1, y, x + w - 1, y + h, c);
		g.fill(x, y + 1, x + 1, y + h - 1, c);
		g.fill(x + w - 1, y + 1, x + w, y + h - 1, c);
	}

	/** Colour for a remaining fraction: normal, then amber, then red. */
	public static int levelColor(float fraction) {
		return fraction > 0.5f ? Theme.TEXT : fraction > 0.2f ? Theme.WARN : Theme.DANGER;
	}
}

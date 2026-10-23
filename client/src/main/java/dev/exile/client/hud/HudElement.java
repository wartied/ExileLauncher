package dev.exile.client.hud;

import dev.exile.client.config.ExileConfig;
import net.minecraft.client.Minecraft;

/**
 * One movable HUD element. Elements cache what they display and refresh it in
 * {@link #tick}, so drawing each frame only paints prepared text.
 */
public abstract class HudElement {
	public final String id;
	public final String name;
	public final String description;
	private final boolean defaultOn;
	private final float defaultX, defaultY;

	protected HudElement(String id, String name, String description, boolean defaultOn, float defaultX, float defaultY) {
		this.id = id;
		this.name = name;
		this.description = description;
		this.defaultOn = defaultOn;
		this.defaultX = defaultX;
		this.defaultY = defaultY;
	}

	public ExileConfig.HudSlot slot() {
		return ExileConfig.get().slot(id, defaultOn, defaultX, defaultY);
	}

	public boolean enabled() {
		return slot().enabled;
	}

	/** Called every client tick while enabled; refresh cached values here. */
	public void tick(Minecraft mc) {}

	/** Whether there is anything to show right now (e.g. no active effects). */
	public boolean hasContent(Minecraft mc) {
		return mc.player != null;
	}

	/** Current size in GUI pixels. */
	public abstract int width();

	public abstract int height();

	/** Draws at the given top-left corner. {@code preview} is set in the layout editor. */
	public abstract void draw(HudDraw d, int x, int y, boolean preview);
}

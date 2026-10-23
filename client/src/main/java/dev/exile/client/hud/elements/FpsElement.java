package dev.exile.client.hud.elements;

import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.perf.FrameStats;
import dev.exile.client.ui.Theme;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;

/** Frame rate, refreshed twice a second so the number is readable. */
public final class FpsElement extends HudElement {
	private Component label = Labels.valueUnit("0", " fps");
	private int shown = -1;
	private int ticks;

	public FpsElement() {
		super("fps", "FPS", "Frames per second", true, 0f, 0f);
	}

	@Override
	public void tick(Minecraft mc) {
		if (ticks++ % 10 != 0) return;
		double ms = FrameStats.smoothedFrameMs();
		int fps = ms > 0 ? (int) Math.round(1000.0 / ms) : 0;
		if (fps != shown) {
			shown = fps;
			label = Labels.valueUnit(Integer.toString(fps), " fps");
		}
	}

	@Override
	public int width() {
		return Minecraft.getInstance().font.width(label);
	}

	@Override
	public int height() {
		return 8;
	}

	@Override
	public void draw(HudDraw d, int x, int y, boolean preview) {
		d.text(label, x, y, Theme.TEXT);
	}
}

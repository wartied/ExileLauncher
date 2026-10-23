package dev.exile.client.hud.elements;

import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.ui.Theme;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;

/** Food level and the normally hidden saturation value, each with a bar. */
public final class SaturationElement extends HudElement {
	private static final int BAR = 64;
	private int food = -1;
	private float saturation = -1;
	private Component foodLabel = Labels.dim("Food");
	private Component satLabel = Labels.dim("Saturation");

	public SaturationElement() {
		super("saturation", "Saturation", "Food level and saturation (normally hidden)", true, 1f, 1f);
	}

	@Override
	public void tick(Minecraft mc) {
		var fd = mc.player.getFoodData();
		int f = fd.getFoodLevel();
		float s = Math.round(fd.getSaturationLevel() * 10f) / 10f;
		if (f != food) {
			food = f;
			foodLabel = Labels.line().append(Labels.dim("Food  ")).append(Theme.bold(Integer.toString(f)));
		}
		if (s != saturation) {
			saturation = s;
			satLabel = Labels.line().append(Labels.dim("Saturation  ")).append(Theme.bold(String.format("%.1f", s)));
		}
	}

	@Override
	public int width() {
		var f = Minecraft.getInstance().font;
		return Math.max(BAR, Math.max(f.width(foodLabel), f.width(satLabel)));
	}

	@Override
	public int height() {
		return 30;
	}

	@Override
	public void draw(HudDraw d, int x, int y, boolean preview) {
		int f = Math.max(0, food);
		float s = Math.max(0, saturation);
		d.text(foodLabel, x, y, Theme.TEXT);
		bar(d, x, y + 10, f / 20f, HudDraw.levelColor(f / 20f));
		d.text(satLabel, x, y + 16, Theme.TEXT);
		bar(d, x, y + 26, s / 20f, Theme.WARN);
	}

	private static void bar(HudDraw d, int x, int y, float fraction, int color) {
		d.fill(x, y, BAR, 2, Theme.alpha(Theme.SURFACE_ACTIVE, 0.9f));
		d.fill(x, y, Math.round(BAR * Math.clamp(fraction, 0f, 1f)), 2, color);
	}
}

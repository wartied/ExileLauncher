package dev.exile.client.hud.elements;

import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.hud.SaturationOverlay;
import dev.exile.client.ui.Theme;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;

/** Food level and the normally hidden saturation value, each with a bar. */
public final class SaturationElement extends HudElement {
	private static final int BAR = 64;
	private static final Identifier FOOD_EMPTY = Identifier.withDefaultNamespace("hud/food_empty");
	private static final Identifier FOOD_FULL = Identifier.withDefaultNamespace("hud/food_full");
	private int food = -1;
	private float saturation = -1;
	private Component foodLabel = Labels.dim("Food");
	private Component satLabel = Labels.dim("Saturation");
	private Component foodValue = Theme.bold("20");
	private Component satValue = Theme.bold("5.0");

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
			foodValue = Theme.bold(Integer.toString(f));
			foodLabel = Labels.line().append(Labels.dim("Food  ")).append(foodValue);
		}
		if (s != saturation) {
			saturation = s;
			satValue = Theme.bold(String.format("%.1f", s));
			satLabel = Labels.line().append(Labels.dim("Saturation  ")).append(satValue);
		}
	}

	@Override
	public List<Mode> modes() {
		return List.of(Mode.FULL, Mode.ICON, Mode.TEXT);
	}

	@Override
	public int width() {
		var f = Minecraft.getInstance().font;
		return switch (mode()) {
			// Drumstick and value, then the outlined drumstick and value.
			case ICON -> 12 + f.width(foodValue) + 8 + 12 + f.width(satValue);
			case TEXT -> Math.max(f.width(foodLabel), f.width(satLabel));
			case FULL -> Math.max(BAR, Math.max(f.width(foodLabel), f.width(satLabel)));
		};
	}

	@Override
	public int height() {
		return switch (mode()) {
			case ICON -> 9;
			case TEXT -> 19;
			case FULL -> 30;
		};
	}

	@Override
	public void draw(HudDraw d, int x, int y, boolean preview) {
		int f = Math.max(0, food);
		float s = Math.max(0, saturation);
		switch (mode()) {
			case ICON -> {
				d.g.blitSprite(RenderPipelines.GUI_TEXTURED, FOOD_EMPTY, x, y, 9, 9);
				d.g.blitSprite(RenderPipelines.GUI_TEXTURED, FOOD_FULL, x, y, 9, 9);
				d.text(foodValue, x + 12, y + 1, Theme.TEXT);
				int sx = x + 12 + d.width(foodValue) + 8;
				d.g.blitSprite(RenderPipelines.GUI_TEXTURED, FOOD_EMPTY, sx, y, 9, 9);
				SaturationOverlay.outline(d.g, sx, y, Theme.WARN);
				d.text(satValue, sx + 12, y + 1, Theme.TEXT);
			}
			case TEXT -> {
				d.text(foodLabel, x, y, Theme.TEXT);
				d.text(satLabel, x, y + 10, Theme.TEXT);
			}
			case FULL -> {
				d.text(foodLabel, x, y, Theme.TEXT);
				bar(d, x, y + 10, f / 20f, HudDraw.levelColor(f / 20f));
				d.text(satLabel, x, y + 16, Theme.TEXT);
				bar(d, x, y + 26, s / 20f, Theme.WARN);
			}
		}
	}

	private static void bar(HudDraw d, int x, int y, float fraction, int color) {
		d.fill(x, y, BAR, 2, Theme.alpha(Theme.SURFACE_ACTIVE, 0.9f));
		d.fill(x, y, Math.round(BAR * Math.clamp(fraction, 0f, 1f)), 2, color);
	}
}

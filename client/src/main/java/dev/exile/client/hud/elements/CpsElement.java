package dev.exile.client.hud.elements;

import dev.exile.client.feature.ClickCounter;
import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.ui.Theme;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;

/** Left and right clicks over the last second. */
public final class CpsElement extends HudElement {
	private Component label = Labels.valueUnit("0 | 0", " cps");
	private int lastLeft = -1, lastRight = -1;

	public CpsElement() {
		super("cps", "CPS counter", "Clicks per second, left | right", false, 0f, 0f);
	}

	@Override
	public void tick(Minecraft mc) {
		int l = ClickCounter.leftCps(), r = ClickCounter.rightCps();
		if (l != lastLeft || r != lastRight) {
			lastLeft = l;
			lastRight = r;
			label = Labels.valueUnit(l + " | " + r, " cps");
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

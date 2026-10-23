package dev.exile.client.hud.elements;

import dev.exile.client.feature.ClickCounter;
import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.ui.Theme;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;

/** W A S D, mouse buttons with clicks per second, and jump. Pressed keys light up. */
public final class KeystrokesElement extends HudElement {
	private static final int KEY = 18;
	private static final int GAP = 2;
	private static final int WIDTH = KEY * 3 + GAP * 2;
	private static final Component W = Theme.bold("W"), A = Theme.bold("A"), S = Theme.bold("S"), D = Theme.bold("D");
	private Component lmb = Theme.bold("LMB"), rmb = Theme.bold("RMB");
	private int lastLeft = -1, lastRight = -1;

	public KeystrokesElement() {
		super("keystrokes", "Keystrokes", "Movement keys and mouse buttons as you press them", false, 0f, 1f);
	}

	@Override
	public void tick(Minecraft mc) {
		int l = ClickCounter.leftCps(), r = ClickCounter.rightCps();
		if (l != lastLeft) {
			lastLeft = l;
			lmb = l > 0 ? Theme.bold(l + " CPS") : Theme.bold("LMB");
		}
		if (r != lastRight) {
			lastRight = r;
			rmb = r > 0 ? Theme.bold(r + " CPS") : Theme.bold("RMB");
		}
	}

	@Override
	public int width() {
		return WIDTH;
	}

	@Override
	public int height() {
		return KEY * 3 + GAP * 3 + 8;
	}

	@Override
	public void draw(HudDraw d, int x, int y, boolean preview) {
		var o = Minecraft.getInstance().options;
		key(d, x + KEY + GAP, y, KEY, KEY, W, o.keyUp);
		int row2 = y + KEY + GAP;
		key(d, x, row2, KEY, KEY, A, o.keyLeft);
		key(d, x + KEY + GAP, row2, KEY, KEY, S, o.keyDown);
		key(d, x + (KEY + GAP) * 2, row2, KEY, KEY, D, o.keyRight);
		int row3 = row2 + KEY + GAP;
		int half = (WIDTH - GAP) / 2;
		key(d, x, row3, half, KEY, lmb, o.keyAttack);
		key(d, x + half + GAP, row3, WIDTH - half - GAP, KEY, rmb, o.keyUse);
		int row4 = row3 + KEY + GAP;
		boolean jump = o.keyJump.isDown();
		d.fill(x, row4, WIDTH, 8, jump ? Theme.ACCENT : Theme.alpha(Theme.SURFACE_ACTIVE, 0.85f));
		d.fill(x + WIDTH / 2 - 8, row4 + 3, 16, 2, jump ? 0xFFFFFFFF : Theme.TEXT_DIM);
	}

	private static void key(HudDraw d, int x, int y, int w, int h, Component label, KeyMapping mapping) {
		boolean down = mapping.isDown();
		d.fill(x, y, w, h, down ? Theme.ACCENT : Theme.alpha(Theme.SURFACE_ACTIVE, 0.85f));
		d.text(label, x + (w - d.width(label)) / 2, y + (h - 7) / 2, down ? 0xFFFFFFFF : Theme.TEXT);
	}
}

package dev.exile.client.hud.elements;

import dev.exile.client.hud.HudDraw;
import dev.exile.client.hud.HudElement;
import dev.exile.client.ui.Theme;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import net.minecraft.util.Mth;

/** Block coordinates and facing direction. */
public final class CoordinatesElement extends HudElement {
	// Indexed by yaw / 45 degrees: 0 = south in Minecraft's convention.
	private static final String[] DIRS = {"S", "SW", "W", "NW", "N", "NE", "E", "SE"};
	private static final String[] AXES = {"+Z", "-X +Z", "-X", "-X -Z", "-Z", "+X -Z", "+X", "+X +Z"};
	private Component line1 = Labels.dim("XYZ");
	private Component line2 = Labels.dim("Facing");
	private int lx = Integer.MIN_VALUE, ly, lz, ldir = -1;

	public CoordinatesElement() {
		super("coordinates", "Coordinates", "Block position and facing direction", true, 0f, 0f);
	}

	@Override
	public void tick(Minecraft mc) {
		var p = mc.player;
		int x = Mth.floor(p.getX()), y = Mth.floor(p.getY()), z = Mth.floor(p.getZ());
		int dir = Math.floorMod(Math.round(Mth.wrapDegrees(p.getYRot()) / 45f), 8);
		if (x != lx || y != ly || z != lz) {
			lx = x;
			ly = y;
			lz = z;
			line1 = Labels.line().append(Labels.dim("XYZ  ")).append(Theme.bold(x + "  " + y + "  " + z));
		}
		if (dir != ldir) {
			ldir = dir;
			line2 = Labels.line().append(Labels.dim("Facing  ")).append(Theme.bold(DIRS[dir])).append(Labels.dim("  " + AXES[dir]));
		}
	}

	@Override
	public int width() {
		var f = Minecraft.getInstance().font;
		return Math.max(f.width(line1), f.width(line2));
	}

	@Override
	public int height() {
		return 18;
	}

	@Override
	public void draw(HudDraw d, int x, int y, boolean preview) {
		d.text(line1, x, y, Theme.TEXT);
		d.text(line2, x, y + 10, Theme.TEXT);
	}
}

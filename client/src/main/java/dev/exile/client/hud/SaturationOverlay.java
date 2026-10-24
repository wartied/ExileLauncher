package dev.exile.client.hud;

import dev.exile.client.config.ExileConfig;
import dev.exile.client.ui.Theme;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;

/**
 * Saturation on the hunger bar: a gold outline around each drumstick that
 * saturation still backs, so you can see how long until the bar starts to
 * drop. A partly used drumstick's outline fades in proportion.
 *
 * <p>Drawn straight after vanilla's food bar, at vanilla's positions (right
 * to left from the centre, 8 px apart). Vanilla only jiggles the icons when
 * saturation is zero, and then there is nothing to outline, so the overlay
 * can never drift from the icons.
 */
public final class SaturationOverlay {
	/** The drumstick's outline, one row per pixel row, bit 8 = leftmost pixel. */
	private static final int[] OUTLINE = {
		0b001100000,
		0b010010000,
		0b100001000,
		0b100000100,
		0b010000100,
		0b001000100,
		0b000111011,
		0b000000101,
		0b000000110,
	};

	private SaturationOverlay() {}

	public static void draw(GuiGraphicsExtractor g) {
		Minecraft mc = Minecraft.getInstance();
		if (!ExileConfig.get().features.saturationOverlay || mc.gui.hud.isHidden()) return;
		if (mc.gameMode == null || !mc.gameMode.canHurtPlayer() || !(mc.getCameraEntity() instanceof Player player)) return;
		// Riding something with health replaces the food bar with its hearts.
		if (player.getVehicle() instanceof LivingEntity vehicle && vehicle.showVehicleHealth()) return;

		float saturation = Math.min(player.getFoodData().getSaturationLevel(), player.getFoodData().getFoodLevel());
		if (saturation <= 0) return;
		int xRight = g.guiWidth() / 2 + 91;
		int y = g.guiHeight() - 39;
		for (int i = 0; i < 10; i++) {
			float points = Math.clamp(saturation - i * 2, 0f, 2f);
			if (points <= 0) break;
			outline(g, xRight - i * 8 - 9, y, Theme.alpha(Theme.WARN, points / 2f));
		}
	}

	private static void outline(GuiGraphicsExtractor g, int x, int y, int color) {
		for (int row = 0; row < OUTLINE.length; row++) {
			int bits = OUTLINE[row];
			// One rectangle per horizontal run of pixels.
			for (int col = 0; col < 9; ) {
				if ((bits & (1 << (8 - col))) == 0) {
					col++;
					continue;
				}
				int start = col;
				while (col < 9 && (bits & (1 << (8 - col))) != 0) col++;
				g.fill(x + start, y + row, x + col, y + row + 1, color);
			}
		}
	}
}
